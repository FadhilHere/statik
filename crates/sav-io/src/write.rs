//! `.sav` writer: little-endian, bytecode-compressed, UTF-8.
use crate::compress::{Compressor, BIAS};
use crate::records::*;
use crate::SavError;
use dataset::{Column, Dataset, Measure, MissingSpec, VarId, VarType, Variable};
use std::collections::HashSet;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_STRING_WIDTH: usize = 32767;
const MAX_VAR_LABEL: usize = 255;
const MAX_VALUE_LABEL: usize = 120;
/// Words SPSS does not accept as variable names.
const RESERVED: [&str; 13] = ["ALL", "AND", "BY", "EQ", "GE", "GT", "LE", "LT", "NE", "NOT", "OR", "TO", "WITH"];

/// A dataset variable as laid out in the file.
struct Planned<'d> {
    var: &'d Variable,
    col: &'d Column,
    /// 0 = numeric, otherwise the string width in bytes.
    width: usize,
    /// Short name of each segment (one unless a very long string).
    short_names: Vec<String>,
    /// 1-based dictionary index of the first segment.
    dict_index: usize,
}

pub fn write_sav(ds: &Dataset, path: &Path) -> Result<(), SavError> {
    let bytes = to_bytes(ds)?;
    std::fs::write(path, bytes)?;
    Ok(())
}

fn to_bytes(ds: &Dataset) -> Result<Vec<u8>, SavError> {
    let plan = plan(ds)?;
    let n_cases = i32::try_from(ds.n_cases()).map_err(|_| SavError::Unsupported("more than 2^31 cases".into()))?;
    let total_slots: usize = plan.iter().map(|p| var_slots(p.width)).sum();
    let mut w = Writer::default();

    // Record 1: file header.
    w.bytes(b"$FL2");
    w.padded(b"@(#) SPSS DATA FILE - Statik", 60);
    w.i32(2); // layout code: little-endian
    w.i32(i32::try_from(total_slots).map_err(|_| SavError::Unsupported("too many variables".into()))?);
    w.i32(1); // bytecode compression
    w.i32(0); // no weight variable
    w.i32(n_cases);
    w.f64(BIAS);
    let (date, time) = creation_stamp();
    w.padded(date.as_bytes(), 9);
    w.padded(time.as_bytes(), 8);
    w.padded(b"", 64); // file label
    w.bytes(&[0; 3]);

    // Record 2: variables (plus continuation records for strings).
    for p in &plan {
        write_variable(&mut w, p);
    }

    // Records 3 + 4: numeric value labels, one set per variable.
    for p in plan.iter().filter(|p| p.width == 0 && !p.var.value_labels.is_empty()) {
        w.i32(REC_VALUE_LABELS);
        w.i32(p.var.value_labels.len() as i32);
        for (value, label) in &p.var.value_labels {
            w.f64(value.0);
            let text = truncate(label, MAX_VALUE_LABEL);
            w.bytes(&[text.len() as u8]);
            w.bytes(text.as_bytes());
            w.bytes(&vec![b' '; (8 - (text.len() + 1) % 8) % 8]);
        }
        w.i32(REC_VALUE_LABEL_VARS);
        w.i32(1);
        w.i32(p.dict_index as i32);
    }

    // Record 7 subtype 3: machine integer info.
    w.extension(SUB_INTEGER_INFO, 4, 8);
    for v in [20, 0, 0, -1, 1, 1, 2, 65001] {
        w.i32(v); // version 20.0.0, machine code, IEEE, compression, little-endian, UTF-8
    }
    // Subtype 4: machine floating-point info.
    w.extension(SUB_FLOAT_INFO, 8, 3);
    w.f64(SYSMIS);
    w.f64(HIGHEST);
    w.f64(f64::from_bits(LOWEST_BITS));
    // Subtype 11: measure, display width, alignment for every segment.
    let n_segments: usize = plan.iter().map(|p| p.short_names.len()).sum();
    w.extension(SUB_DISPLAY, 4, 3 * n_segments);
    for p in &plan {
        let measure = match p.var.measure {
            Measure::Nominal => 1,
            Measure::Ordinal => 2,
            Measure::Scale => 3,
        };
        let columns = p.var.width.clamp(1, 255) as i32;
        let align = if p.width == 0 { 1 } else { 0 }; // right for numbers, left for strings
        for k in 0..p.short_names.len() {
            w.i32(measure);
            w.i32(if k == 0 { columns } else { 8 });
            w.i32(align);
        }
    }
    // Subtype 13: long variable names.
    let long_names: Vec<String> = plan.iter().map(|p| format!("{}={}", p.short_names[0], p.var.name)).collect();
    w.text_extension(SUB_LONG_NAMES, long_names.join("\t").as_bytes());
    // Subtype 14: very long strings.
    let very_long: String = plan
        .iter()
        .filter(|p| p.width > 255)
        .map(|p| format!("{}={:05}\0\t", p.short_names[0], p.width))
        .collect();
    if !very_long.is_empty() {
        w.text_extension(SUB_VERY_LONG_STRINGS, very_long.as_bytes());
    }
    // Subtype 20: character encoding.
    w.text_extension(SUB_ENCODING, b"UTF-8");
    // Record 999: end of dictionary.
    w.i32(REC_END);
    w.i32(0);

    let mut c = Compressor::default();
    for row in 0..ds.n_cases() {
        for p in &plan {
            match p.col {
                Column::Numeric(v) => c.number(v[row]),
                Column::Text(v) => {
                    let mut bytes = v[row].as_bytes().to_vec();
                    bytes.resize(p.width, b' ');
                    let n = p.short_names.len();
                    for k in 0..n {
                        let mut seg = bytes[segment_range(p.width, k)].to_vec();
                        seg.resize(slots(segment_width(p.width, k)) * 8, b' ');
                        for chunk in seg.chunks_exact(8) {
                            c.chunk(chunk.try_into().expect("8 bytes"));
                        }
                    }
                }
            }
        }
    }
    w.buf.extend_from_slice(&c.finish());
    Ok(w.buf)
}

fn plan(ds: &Dataset) -> Result<Vec<Planned<'_>>, SavError> {
    let mut taken: HashSet<String> = HashSet::new();
    let mut dict_index = 1;
    let mut out = Vec::with_capacity(ds.n_vars());
    for i in 0..ds.n_vars() {
        let id = VarId(i);
        let var = ds.var(id);
        let col = ds.column(id);
        let width = match (&var.var_type, col) {
            (VarType::Numeric, _) => 0,
            (VarType::String { width }, Column::Text(vals)) => {
                let longest = vals.iter().map(String::len).max().unwrap_or(0);
                (*width).max(longest).max(1)
            }
            (VarType::String { .. }, Column::Numeric(_)) => {
                return Err(SavError::Format(format!("variable '{}' has a numeric column", var.name)));
            }
        };
        if width > MAX_STRING_WIDTH {
            return Err(SavError::Unsupported(format!(
                "string variable '{}' is wider than {MAX_STRING_WIDTH} bytes",
                var.name
            )));
        }
        if let MissingSpec::Discrete(v) = &var.missing
            && v.len() > 3
        {
            return Err(SavError::Unsupported(format!(
                "variable '{}' has more than 3 discrete missing values",
                var.name
            )));
        }
        let n_segments = if width == 0 { 1 } else { segment_count(width) };
        let short_names = (0..n_segments).map(|_| short_name(&var.name, &mut taken)).collect();
        out.push(Planned { var, col, width, short_names, dict_index });
        dict_index += var_slots(width);
    }
    Ok(out)
}

/// 8-byte slots used by a whole variable (all segments).
fn var_slots(width: usize) -> usize {
    if width == 0 {
        1
    } else {
        (0..segment_count(width)).map(|k| slots(segment_width(width, k))).sum()
    }
}

/// Unique (case-insensitive) upper-case name of at most 8 bytes derived from `name`.
fn short_name(name: &str, taken: &mut HashSet<String>) -> String {
    let upper = name.to_uppercase();
    let prefix = |max: usize| -> String {
        let mut s = String::new();
        for ch in upper.chars() {
            if s.len() + ch.len_utf8() > max {
                break;
            }
            s.push(ch);
        }
        s
    };
    let mut candidate = prefix(8);
    let mut n = 1;
    while candidate.is_empty() || taken.contains(&candidate) || RESERVED.contains(&candidate.as_str()) {
        let suffix = format!("_{n}");
        candidate = prefix(8 - suffix.len());
        if candidate.is_empty() {
            candidate = "V".to_string();
        }
        candidate += &suffix;
        n += 1;
    }
    taken.insert(candidate.clone());
    candidate
}

fn write_variable(w: &mut Writer, p: &Planned) {
    let var = p.var;
    for (k, short) in p.short_names.iter().enumerate() {
        let seg_width = if p.width == 0 { 0 } else { segment_width(p.width, k) };
        let format = if p.width == 0 {
            let width = var.width.clamp(1, 40);
            let decimals = var.decimals.min(16).min(width.saturating_sub(1));
            make_format(FMT_F, width, decimals)
        } else {
            make_format(FMT_A, seg_width, 0)
        };
        let first = k == 0;
        let label = var.label.as_deref().filter(|_| first).map(|l| truncate(l, MAX_VAR_LABEL));
        let missing: Vec<f64> = match (&var.missing, first && p.width == 0) {
            (MissingSpec::Discrete(v), true) => v.clone(),
            (MissingSpec::Range { lo, hi, discrete }, true) => {
                let lo = if *lo == f64::NEG_INFINITY { f64::from_bits(LOWEST_BITS) } else { *lo };
                let hi = if *hi == f64::INFINITY { HIGHEST } else { *hi };
                [lo, hi].into_iter().chain(*discrete).collect()
            }
            _ => Vec::new(),
        };
        let n_missing = match &var.missing {
            MissingSpec::Range { .. } if !missing.is_empty() => -(missing.len() as i32),
            _ => missing.len() as i32,
        };
        w.i32(REC_VARIABLE);
        w.i32(seg_width as i32);
        w.i32(i32::from(label.is_some()));
        w.i32(n_missing);
        w.i32(format);
        w.i32(format);
        w.padded(short.as_bytes(), 8);
        if let Some(l) = label {
            w.i32(l.len() as i32);
            w.bytes(l.as_bytes());
            w.bytes(&vec![b' '; (4 - l.len() % 4) % 4]);
        }
        for m in missing {
            w.f64(m);
        }
        for _ in 1..slots(seg_width) {
            w.i32(REC_VARIABLE);
            w.i32(-1);
            for _ in 0..4 {
                w.i32(0);
            }
            w.bytes(&[0; 8]);
        }
    }
}

/// Longest prefix of `s` that fits in `max` bytes without splitting a character.
fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    let mut end = max;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// ("dd Mon yy", "hh:mm:ss") in UTC.
fn creation_stamp() -> (String, String) {
    const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let (days, rem) = (secs / 86_400, secs % 86_400);
    // Civil-from-days (Howard Hinnant's algorithm).
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    (
        format!("{day:02} {} {:02}", MONTHS[(month - 1) as usize], year % 100),
        format!("{:02}:{:02}:{:02}", rem / 3600, rem % 3600 / 60, rem % 60),
    )
}

#[derive(Default)]
struct Writer {
    buf: Vec<u8>,
}

impl Writer {
    fn bytes(&mut self, b: &[u8]) {
        self.buf.extend_from_slice(b);
    }
    fn i32(&mut self, v: i32) {
        self.bytes(&v.to_le_bytes());
    }
    fn f64(&mut self, v: f64) {
        self.bytes(&v.to_le_bytes());
    }
    /// `b` padded with spaces (or cut) to exactly `len` bytes.
    fn padded(&mut self, b: &[u8], len: usize) {
        let n = b.len().min(len);
        self.bytes(&b[..n]);
        self.bytes(&vec![b' '; len - n]);
    }
    fn extension(&mut self, subtype: i32, size: i32, count: usize) {
        self.i32(REC_EXTENSION);
        self.i32(subtype);
        self.i32(size);
        self.i32(count as i32);
    }
    fn text_extension(&mut self, subtype: i32, text: &[u8]) {
        self.extension(subtype, 1, text.len());
        self.bytes(text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_names_are_unique_and_short() {
        let mut taken = HashSet::new();
        let names: Vec<String> = ["pendapatan_bulanan", "pendapatan_tahunan", "to", "ÄÖÜäöüßx", "a"]
            .iter()
            .map(|n| short_name(n, &mut taken))
            .collect();
        assert_eq!(names[0], "PENDAPAT");
        assert_eq!(names[1], "PENDAP_1");
        assert_eq!(names[2], "TO_1");
        assert!(names[3].len() <= 8, "{}", names[3]);
        assert_eq!(names[4], "A");
    }

    #[test]
    fn truncates_on_char_boundary() {
        assert_eq!(truncate("ééé", 5), "éé");
        assert_eq!(truncate("abc", 5), "abc");
    }
}
