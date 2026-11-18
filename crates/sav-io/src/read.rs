//! `.sav` reader.
use crate::compress::{Decompressor, Slot};
use crate::records::*;
use crate::SavError;
use dataset::{validate_name, Column, Dataset, Measure, MissingSpec, OrderedValue, Variable};
use encoding_rs::{Encoding, WINDOWS_1252};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

/// One type-2 record that is not a continuation.
struct PhysVar {
    /// 0 = numeric, otherwise the string width of this segment.
    type_width: usize,
    name: Vec<u8>,
    label: Option<Vec<u8>>,
    n_missing: i32,
    missing: Vec<[u8; 8]>,
    print: i32,
    /// 1-based dictionary index (counting continuation records).
    dict_index: usize,
    /// Offset of the first 8-byte slot within a case.
    slot: usize,
}

struct ValueLabelSet {
    labels: Vec<([u8; 8], Vec<u8>)>,
    vars: Vec<usize>,
}

#[derive(Default)]
struct Extensions {
    char_code: Option<i32>,
    float_rep: Option<i32>,
    sysmis: Option<f64>,
    display: Option<Vec<i32>>,
    long_names: Option<Vec<u8>>,
    very_long: Option<Vec<u8>>,
    encoding: Option<Vec<u8>>,
}

/// A variable as the user sees it (very long strings join several segments).
struct LogicalVar {
    first: usize,
    n_segments: usize,
    /// 0 = numeric, otherwise the full string width in bytes.
    width: usize,
}

pub fn read_sav(path: &Path) -> Result<Dataset, SavError> {
    let bytes = std::fs::read(path)?;
    parse(&bytes)
}

fn parse(bytes: &[u8]) -> Result<Dataset, SavError> {
    if bytes.len() < 4 {
        return Err(format_error("file too short to be an SPSS system file"));
    }
    match &bytes[..4] {
        b"$FL2" => {}
        b"$FL3" => return Err(SavError::Unsupported("ZSAV".into())),
        _ => return Err(format_error("not an SPSS system file (missing $FL2 signature)")),
    }
    if bytes.len() < HEADER_LEN {
        return Err(format_error("file header is truncated"));
    }
    let layout: [u8; 4] = bytes[64..68].try_into().expect("4 bytes");
    let endian = if matches!(i32::from_le_bytes(layout), 2 | 3) {
        Endian::Little
    } else if matches!(i32::from_be_bytes(layout), 2 | 3) {
        Endian::Big
    } else {
        return Err(format_error("unrecognised layout code"));
    };
    let mut cur = Cursor::new(bytes, 68, endian);
    let _nominal_case_size = cur.i32()?;
    let compression = cur.i32()?;
    let _weight_index = cur.i32()?;
    let header_cases = cur.i32()?;
    let bias = cur.f64()?;
    match compression {
        0 | 1 => {}
        2 => return Err(SavError::Unsupported("ZSAV".into())),
        c => return Err(format_error(format!("unknown compression code {c}"))),
    }
    let mut cur = Cursor::new(bytes, HEADER_LEN, endian);

    let mut phys: Vec<PhysVar> = Vec::new();
    let mut label_sets: Vec<ValueLabelSet> = Vec::new();
    let mut ext = Extensions::default();
    let mut dict_len = 0usize; // records of type 2 seen, including continuations
    let mut pending_continuations = 0usize;
    loop {
        let rec = cur.i32()?;
        match rec {
            REC_VARIABLE => {
                let type_code = cur.i32()?;
                let has_label = cur.i32()?;
                let n_missing = cur.i32()?;
                let print = cur.i32()?;
                let _write = cur.i32()?;
                let name = cur.take(8)?.to_vec();
                dict_len += 1;
                if type_code == -1 {
                    if pending_continuations == 0 {
                        return Err(format_error("unexpected string continuation record"));
                    }
                    pending_continuations -= 1;
                    if has_label != 0 || n_missing != 0 {
                        return Err(format_error("continuation record with label or missing values"));
                    }
                    continue;
                }
                if pending_continuations != 0 {
                    return Err(format_error("string variable is missing continuation records"));
                }
                let type_width = usize::try_from(type_code)
                    .ok()
                    .filter(|&w| w <= 255)
                    .ok_or_else(|| format_error(format!("invalid variable type {type_code}")))?;
                let label = match has_label {
                    0 => None,
                    1 => {
                        let len = cur.count(1, "variable label")?;
                        let text = cur.take(len)?.to_vec();
                        cur.take((4 - len % 4) % 4)?;
                        Some(text)
                    }
                    _ => return Err(format_error("invalid variable label flag")),
                };
                let n_values = match n_missing {
                    0..=3 => n_missing as usize,
                    -2 => 2,
                    -3 => 3,
                    _ => return Err(format_error(format!("invalid missing value count {n_missing}"))),
                };
                let missing = (0..n_values).map(|_| cur.take8()).collect::<Result<_, _>>()?;
                pending_continuations = slots(type_width) - 1;
                let slot = phys.last().map_or(0, |p: &PhysVar| p.slot + slots(p.type_width));
                phys.push(PhysVar { type_width, name, label, n_missing, missing, print, dict_index: dict_len, slot });
            }
            REC_VALUE_LABELS => {
                let n = cur.count(9, "value label")?;
                let mut labels = Vec::with_capacity(n);
                for _ in 0..n {
                    let value = cur.take8()?;
                    let len = usize::from(cur.take(1)?[0]);
                    let text = cur.take(len)?.to_vec();
                    cur.take((8 - (len + 1) % 8) % 8)?;
                    labels.push((value, text));
                }
                if cur.i32()? != REC_VALUE_LABEL_VARS {
                    return Err(format_error("value label record not followed by its variable list"));
                }
                let n = cur.count(4, "value label variable")?;
                let vars = (0..n)
                    .map(|_| cur.i32().map(|i| usize::try_from(i).unwrap_or(0)))
                    .collect::<Result<_, _>>()?;
                label_sets.push(ValueLabelSet { labels, vars });
            }
            REC_DOCUMENT => {
                let n = cur.count(80, "document line")?;
                cur.take(n * 80)?;
            }
            REC_EXTENSION => {
                let subtype = cur.i32()?;
                let size = cur.count(0, "extension size")?;
                let count = cur.count(size, "extension")?;
                let data = cur.take(size * count)?;
                read_extension(subtype, size, count, data, endian, &mut ext);
            }
            REC_END => {
                cur.i32()?;
                break;
            }
            other => return Err(format_error(format!("unknown record type {other}"))),
        }
    }
    if pending_continuations != 0 {
        return Err(format_error("string variable is missing continuation records"));
    }
    if ext.float_rep.is_some_and(|r| r != 1) {
        return Err(SavError::Unsupported("non-IEEE floating point".into()));
    }

    let encoding = pick_encoding(&ext)?;
    let decode = |b: &[u8]| decode_label(encoding, b);
    let logical = group_very_long_strings(&phys, &ext, encoding)?;
    let sysmis = ext.sysmis.unwrap_or(SYSMIS);

    // Long names.
    let mut long_names: HashMap<String, String> = HashMap::new();
    if let Some(raw) = &ext.long_names {
        for pair in encoding.decode_without_bom_handling(raw).0.split('\t') {
            if let Some((short, long)) = pair.split_once('=') {
                long_names.insert(short.trim().to_uppercase(), long.trim_end_matches(['\0', ' ']).to_string());
            }
        }
    }

    // Measure per physical variable: subtype 11 has 3 integers (measure, width, alignment)
    // per segment, or 2 (measure, alignment) in old files. Ignored if the count fits neither.
    let measures: Option<Vec<i32>> = ext.display.as_ref().and_then(|data| {
        let per = if phys.is_empty() || data.len() % phys.len() != 0 { 0 } else { data.len() / phys.len() };
        matches!(per, 2 | 3).then(|| data.chunks(per).map(|c| c[0]).collect())
    });

    // Value labels: dictionary index -> logical variable.
    let by_dict: HashMap<usize, usize> =
        logical.iter().enumerate().map(|(i, l)| (phys[l.first].dict_index, i)).collect();
    let mut value_labels: Vec<BTreeMap<OrderedValue, String>> = vec![BTreeMap::new(); logical.len()];
    for set in &label_sets {
        for &idx in &set.vars {
            let li = *by_dict
                .get(&idx)
                .ok_or_else(|| format_error(format!("value labels refer to invalid variable index {idx}")))?;
            if logical[li].width != 0 {
                continue; // string value labels are not part of the data model (KNOWN_GAPS.md)
            }
            for (value, text) in &set.labels {
                let v = f64_from(*value, endian);
                value_labels[li].insert(OrderedValue::new(v), decode_label(encoding, text));
            }
        }
    }

    let mut ds = Dataset::new();
    let mut taken: HashSet<String> = HashSet::new();
    let mut variables = Vec::with_capacity(logical.len());
    for (li, lv) in logical.iter().enumerate() {
        let p = &phys[lv.first];
        let short = decode(&p.name);
        let name = long_names.get(&short.to_uppercase()).cloned().unwrap_or(short);
        let name = unique_valid_name(&name, &mut taken);
        let mut v = if lv.width == 0 {
            let (_, w, d) = split_format(p.print);
            let mut v = Variable::numeric(&name);
            v.width = if w == 0 { 8 } else { w };
            v.decimals = d;
            v.missing = numeric_missing(p, endian);
            v
        } else {
            Variable::string(&name, lv.width)
        };
        v.label = p.label.as_ref().map(|l| decode_label(encoding, l));
        v.value_labels = std::mem::take(&mut value_labels[li]);
        if let Some(m) = measures.as_ref().map(|m| m[lv.first]) {
            match m {
                1 => v.measure = Measure::Nominal,
                2 => v.measure = Measure::Ordinal,
                3 => v.measure = Measure::Scale,
                _ => {}
            }
        }
        variables.push(v);
    }

    let layout = DataLayout { phys: &phys, logical: &logical, sysmis, encoding };
    let columns = read_data(&mut cur, compression == 1, bias, header_cases, layout)?;
    for (v, col) in variables.into_iter().zip(columns) {
        ds.add_variable(v, col).map_err(|e| format_error(e.to_string()))?;
    }
    Ok(ds)
}

fn read_extension(subtype: i32, size: usize, count: usize, data: &[u8], endian: Endian, ext: &mut Extensions) {
    let ints = || {
        data.chunks_exact(4)
            .map(|c| {
                let b: [u8; 4] = c.try_into().expect("4 bytes");
                match endian {
                    Endian::Little => i32::from_le_bytes(b),
                    Endian::Big => i32::from_be_bytes(b),
                }
            })
            .collect::<Vec<_>>()
    };
    match subtype {
        SUB_INTEGER_INFO if size == 4 && count >= 8 => {
            let v = ints();
            ext.float_rep = Some(v[4]);
            ext.char_code = Some(v[7]);
        }
        SUB_FLOAT_INFO if size == 8 && count >= 1 => {
            ext.sysmis = Some(f64_from(data[..8].try_into().expect("8 bytes"), endian));
        }
        SUB_DISPLAY if size == 4 => {
            ext.display = Some(ints());
        }
        SUB_LONG_NAMES if size == 1 => ext.long_names = Some(data.to_vec()),
        SUB_VERY_LONG_STRINGS if size == 1 => ext.very_long = Some(data.to_vec()),
        SUB_ENCODING if size == 1 => ext.encoding = Some(data.to_vec()),
        _ => {} // documents, multiple response sets, long string labels/missing (21/22), etc.
    }
}

fn pick_encoding(ext: &Extensions) -> Result<&'static Encoding, SavError> {
    if let Some(name) = &ext.encoding {
        let label = String::from_utf8_lossy(name);
        let label = label.trim_end_matches(['\0', ' ']);
        return Encoding::for_label(label.as_bytes())
            .ok_or_else(|| SavError::Unsupported(format!("character encoding {label}")));
    }
    Ok(ext.char_code.and_then(code_page_encoding).unwrap_or(WINDOWS_1252))
}

/// Encoding for a Windows code page number (subtype 3 "character code").
/// `None` for unknown pages and for 1–3 (old ASCII/EBCDIC markers), which fall back to Windows-1252.
pub(crate) fn code_page_encoding(code: i32) -> Option<&'static Encoding> {
    let label = match code {
        65001 => "utf-8",
        20127 | 28591 => "windows-1252", // US-ASCII and ISO-8859-1: Windows-1252 is a superset
        874 => "windows-874",
        932 => "shift_jis",
        936 => "gbk",
        949 => "euc-kr",
        950 => "big5",
        1250..=1258 => return Encoding::for_label(format!("windows-{code}").as_bytes()),
        20866 => "koi8-r",
        21866 => "koi8-u",
        28592..=28606 => return Encoding::for_label(format!("iso-8859-{}", code - 28590).as_bytes()),
        51932 | 20932 => "euc-jp",
        51936 => "gbk",
        51949 => "euc-kr",
        54936 => "gb18030",
        _ => return None,
    };
    Encoding::for_label(label.as_bytes())
}

/// Decodes text from the file, dropping the space/NUL padding SPSS adds on the right.
fn decode_label(encoding: &'static Encoding, raw: &[u8]) -> String {
    encoding.decode_without_bom_handling(raw).0.trim_end_matches(['\0', ' ']).to_string()
}

/// Joins the segments of very long strings (subtype 14) into one logical variable each.
fn group_very_long_strings(
    phys: &[PhysVar],
    ext: &Extensions,
    encoding: &'static Encoding,
) -> Result<Vec<LogicalVar>, SavError> {
    let decode = |b: &[u8]| decode_label(encoding, b);
    let mut widths: HashMap<String, usize> = HashMap::new();
    if let Some(raw) = &ext.very_long {
        for entry in encoding.decode_without_bom_handling(raw).0.split('\t') {
            let entry = entry.trim_matches(['\0', ' ']);
            if let Some((short, w)) = entry.split_once('=') {
                let w: usize = w
                    .trim_matches(['\0', ' '])
                    .parse()
                    .map_err(|_| format_error(format!("invalid very long string width '{w}'")))?;
                widths.insert(short.trim().to_uppercase(), w);
            }
        }
    }
    let mut out = Vec::new();
    let mut i = 0;
    while i < phys.len() {
        let p = &phys[i];
        let width = match widths.get(&decode(&p.name).to_uppercase()) {
            Some(&w) if p.type_width != 0 && w > 255 && w <= 32767 => w,
            _ => p.type_width,
        };
        let n = if width == 0 { 1 } else { segment_count(width) };
        if i + n > phys.len() {
            return Err(format_error("very long string is missing segments"));
        }
        for (k, seg) in phys[i..i + n].iter().enumerate() {
            if width != 0 && slots(seg.type_width) != slots(segment_width(width, k)) {
                return Err(format_error("very long string segment has the wrong width"));
            }
        }
        out.push(LogicalVar { first: i, n_segments: n, width });
        i += n;
    }
    Ok(out)
}

/// Missing values of a numeric variable; `LO`/`HI` range ends become ∓infinity.
fn numeric_missing(p: &PhysVar, endian: Endian) -> MissingSpec {
    let vals = &p.missing;
    let num = |b: [u8; 8]| f64_from(b, endian) + 0.0;
    let lo = |b: [u8; 8]| {
        let v = f64_from(b, endian);
        if v.to_bits() == LOWEST_BITS || v == SYSMIS { f64::NEG_INFINITY } else { v + 0.0 }
    };
    let hi = |b: [u8; 8]| if num(b) == HIGHEST { f64::INFINITY } else { num(b) };
    match p.n_missing {
        1..=3 => MissingSpec::Discrete(vals.iter().copied().map(num).collect()),
        -2 => MissingSpec::Range { lo: lo(vals[0]), hi: hi(vals[1]), discrete: None },
        -3 => MissingSpec::Range { lo: lo(vals[0]), hi: hi(vals[1]), discrete: Some(num(vals[2])) },
        _ => MissingSpec::None,
    }
}

/// Long names may be anything the file contains; Statik needs valid, unique names.
fn unique_valid_name(name: &str, taken: &mut HashSet<String>) -> String {
    let mut base: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() || matches!(c, '_' | '.' | '$' | '#' | '@') { c } else { '_' })
        .collect();
    if !base.chars().next().is_some_and(char::is_alphabetic) {
        base.insert(0, 'v');
    }
    let base: String = base.chars().take(64).collect();
    let mut candidate = base.clone();
    let mut n = 1;
    while taken.contains(&candidate.to_lowercase()) || validate_name(&candidate).is_err() {
        let suffix = format!("_{n}");
        candidate = base.chars().take(64 - suffix.chars().count()).collect::<String>() + &suffix;
        n += 1;
    }
    taken.insert(candidate.to_lowercase());
    candidate
}

enum Source<'a, 'b> {
    Compressed(Decompressor<'a, 'b>),
    Raw(&'b mut Cursor<'a>),
}

impl Source<'_, '_> {
    fn next_slot(&mut self) -> Result<Option<Slot>, SavError> {
        match self {
            Source::Compressed(d) => d.next_slot(),
            Source::Raw(c) if c.remaining() == 0 => Ok(None),
            Source::Raw(c) => Ok(Some(Slot::Raw(c.take8()?))),
        }
    }
}

struct DataLayout<'v> {
    phys: &'v [PhysVar],
    logical: &'v [LogicalVar],
    sysmis: f64,
    encoding: &'static Encoding,
}

fn read_data(
    cur: &mut Cursor,
    compressed: bool,
    bias: f64,
    header_cases: i32,
    layout: DataLayout,
) -> Result<Vec<Column>, SavError> {
    let DataLayout { phys, logical, sysmis, encoding } = layout;
    let numeric_slot: Vec<bool> =
        phys.iter().flat_map(|p| std::iter::repeat_n(p.type_width == 0, slots(p.type_width))).collect();
    let n_slots = numeric_slot.len();
    let expected = usize::try_from(header_cases).ok();
    let endian = cur.endian;
    // Never trust the header's case count for allocation: a compressed slot needs at least
    // one command byte and a raw slot 8 bytes, so the rest of the file bounds the case count.
    let min_case_bytes = if compressed { n_slots } else { 8 * n_slots };
    let cap = expected.unwrap_or(0).min(cur.remaining() / min_case_bytes.max(1));
    let mut columns: Vec<Column> = logical
        .iter()
        .map(|l| {
            if l.width == 0 {
                Column::Numeric(Vec::with_capacity(cap))
            } else {
                Column::Text(Vec::with_capacity(cap))
            }
        })
        .collect();
    if n_slots == 0 {
        return Ok(columns);
    }
    let mut src = if compressed { Source::Compressed(Decompressor::new(cur, bias)) } else { Source::Raw(cur) };
    let mut strings: Vec<[u8; 8]> = vec![[b' '; 8]; n_slots];
    let mut numbers: Vec<f64> = vec![0.0; n_slots];
    let mut n_cases = 0usize;
    while expected.is_none_or(|n| n_cases < n) {
        for (s, &is_num) in numeric_slot.iter().enumerate() {
            let Some(item) = src.next_slot()? else {
                if s == 0 && expected.is_none() {
                    return Ok(columns);
                }
                let of = expected.map_or_else(|| "?".to_string(), |n| n.to_string());
                return Err(format_error(format!("case data ends early (case {} of {of})", n_cases + 1)));
            };
            if is_num {
                numbers[s] = match item {
                    Slot::Number(v) => v + 0.0,
                    Slot::Raw(b) => {
                        let v = f64_from(b, endian);
                        if v == sysmis { f64::NAN } else { v + 0.0 }
                    }
                    Slot::Sysmis | Slot::Spaces => f64::NAN,
                };
            } else {
                strings[s] = match item {
                    Slot::Raw(b) => b,
                    _ => [b' '; 8],
                };
            }
        }
        for (lv, col) in logical.iter().zip(columns.iter_mut()) {
            let start = phys[lv.first].slot;
            match col {
                Column::Numeric(v) => v.push(numbers[start]),
                Column::Text(v) => {
                    let mut bytes = Vec::with_capacity(lv.width);
                    for k in 0..lv.n_segments {
                        let seg = &phys[lv.first + k];
                        let seg_bytes = strings[seg.slot..seg.slot + slots(seg.type_width)].concat();
                        let r = segment_range(lv.width, k);
                        bytes.extend_from_slice(&seg_bytes[..r.len().min(seg_bytes.len())]);
                    }
                    v.push(decode_label(encoding, &bytes));
                }
            }
        }
        n_cases += 1;
    }
    Ok(columns)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_pages_map_to_encodings() {
        let name = |c| code_page_encoding(c).map(Encoding::name);
        assert_eq!(name(65001), Some("UTF-8"));
        assert_eq!(name(1252), Some("windows-1252"));
        assert_eq!(name(1250), Some("windows-1250"));
        assert_eq!(name(874), Some("windows-874"));
        assert_eq!(name(932), Some("Shift_JIS"));
        assert_eq!(name(936), Some("GBK"));
        assert_eq!(name(949), Some("EUC-KR"));
        assert_eq!(name(950), Some("Big5"));
        assert_eq!(name(20127), Some("windows-1252"));
        assert_eq!(name(28591), Some("windows-1252"));
        assert_eq!(name(28592), Some("ISO-8859-2"));
        assert_eq!(name(54936), Some("gb18030"));
        assert_eq!(name(2), None);
        assert_eq!(name(12345), None);
    }
}
