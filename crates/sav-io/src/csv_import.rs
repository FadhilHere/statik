//! CSV import with automatic delimiter and decimal-separator detection (Indonesian Excel exports use `;` and `,`).
use crate::SavError;
use crate::names::sanitize_name;
use dataset::{Column, Dataset, Variable};
use std::collections::HashSet;
use std::path::Path;

/// Import options; `None` means "detect automatically".
#[derive(Clone, Debug)]
pub struct CsvOptions {
    pub delimiter: Option<u8>,
    pub decimal_comma: Option<bool>,
    /// The first row holds the variable names.
    pub header: bool,
}

impl Default for CsvOptions {
    fn default() -> Self {
        CsvOptions { delimiter: None, decimal_comma: None, header: true }
    }
}

/// One parsed cell, shared by the CSV and XLSX importers.
#[derive(Clone, Debug)]
pub(crate) enum Cell {
    Empty,
    /// A number: its value, the digits after the decimal separator and the original text
    /// (used when the column turns out to be text).
    Num(f64, usize, String),
    Text(String),
}

/// Parse a number. Without `decimal_comma` only plain numbers (`-12.5`, `1e3`) are accepted. With it the
/// decimal separator is `,` and `.` is accepted as a thousands separator only between groups of 3 digits.
pub(crate) fn parse_number(s: &str, decimal_comma: bool) -> Option<(f64, usize)> {
    let s = s.trim();
    let (neg, rest) = match s.as_bytes().first()? {
        b'-' => (true, &s[1..]),
        b'+' => (false, &s[1..]),
        _ => (false, s),
    };
    let (mant, exp) = match rest.find(['e', 'E']) {
        Some(i) => (&rest[..i], Some(&rest[i + 1..])),
        None => (rest, None),
    };
    let sep = if decimal_comma { ',' } else { '.' };
    let (int_raw, frac) = match mant.split_once(sep) {
        Some((a, b)) if !b.is_empty() => (a, b),
        Some(_) => return None,
        None => (mant, ""),
    };
    let int_digits: String = if decimal_comma && int_raw.contains('.') {
        let groups: Vec<&str> = int_raw.split('.').collect();
        let ok = groups.iter().enumerate().all(|(i, g)| {
            g.bytes().all(|b| b.is_ascii_digit()) && if i == 0 { (1..=3).contains(&g.len()) } else { g.len() == 3 }
        });
        if !ok {
            return None;
        }
        groups.concat()
    } else if int_raw.bytes().all(|b| b.is_ascii_digit()) {
        int_raw.to_string()
    } else {
        return None;
    };
    if !frac.bytes().all(|b| b.is_ascii_digit()) || (int_digits.is_empty() && frac.is_empty()) {
        return None;
    }
    // More than 15 significant digits in an integer would lose precision as f64: keep it as text.
    if frac.is_empty() && exp.is_none() && int_digits.trim_start_matches('0').len() > 15 {
        return None;
    }
    let mut norm = String::new();
    if neg {
        norm.push('-');
    }
    norm.push_str(if int_digits.is_empty() { "0" } else { &int_digits });
    if !frac.is_empty() {
        norm.push('.');
        norm.push_str(frac);
    }
    if let Some(e) = exp {
        let digits = e.strip_prefix(['+', '-']).unwrap_or(e);
        if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        norm.push('e');
        norm.push_str(e);
    }
    let v: f64 = norm.parse().ok()?;
    v.is_finite().then_some((v + 0.0, frac.len()))
}

fn classify(s: &str, decimal_comma: bool) -> Cell {
    if s.trim().is_empty() {
        return Cell::Empty;
    }
    match parse_number(s, decimal_comma) {
        Some((v, d)) => Cell::Num(v, d, s.to_string()),
        None => Cell::Text(s.to_string()),
    }
}

/// Build a dataset from header texts and columns of cells. A column is numeric when none of its cells is text.
pub(crate) fn build_dataset(headers: &[String], cols: Vec<Vec<Cell>>) -> Result<Dataset, SavError> {
    let mut ds = Dataset::new();
    let mut used: HashSet<String> = HashSet::new();
    for (raw, cells) in headers.iter().zip(cols) {
        let name = sanitize_name(raw, &|c: &str| used.contains(&c.to_lowercase()));
        used.insert(name.to_lowercase());
        let numeric = cells.iter().all(|c| !matches!(c, Cell::Text(_)));
        let mut var;
        let col;
        if numeric {
            let mut decimals = 0;
            let mut int_len = 0;
            let mut values = Vec::with_capacity(cells.len());
            for c in &cells {
                match c {
                    Cell::Num(v, d, _) => {
                        decimals = decimals.max(*d);
                        int_len = int_len.max(format!("{:.0}", v.abs().trunc()).len());
                        values.push(*v);
                    }
                    _ => values.push(f64::NAN),
                }
            }
            var = Variable::numeric(&name);
            var.decimals = decimals.min(4);
            var.width = (int_len + var.decimals + 1).max(8);
            col = Column::Numeric(values);
        } else {
            let texts: Vec<String> = cells
                .into_iter()
                .map(|c| match c {
                    Cell::Empty => String::new(),
                    Cell::Num(_, _, t) | Cell::Text(t) => t,
                })
                .collect();
            let width = texts.iter().map(String::len).max().unwrap_or(1).max(1);
            var = Variable::string(&name, width);
            col = Column::Text(texts);
        }
        if raw.trim() != name && !raw.trim().is_empty() {
            var.label = Some(raw.trim().to_string());
        }
        ds.add_variable(var, col).map_err(|e| SavError::Csv(e.to_string()))?; // names are sanitised, so this cannot fail in practice
    }
    Ok(ds)
}

type Record = (usize, Vec<String>);

/// Split `text` into records of fields (RFC 4180 quoting). Each record carries its first line number.
/// Blank lines are skipped. Stops after `max` records when given.
fn parse_records(text: &str, delim: char, max: Option<usize>) -> Result<Vec<Record>, SavError> {
    let mut out: Vec<Record> = Vec::new();
    let mut fields: Vec<String> = Vec::new();
    let mut field = String::new();
    let mut quoted = false; // the current field started with a quote
    let mut in_quotes = false;
    let mut line = 1usize;
    let mut start_line = 1usize;
    let mut chars = text.chars().peekable();

    // `last`: the field ends the line, where unquoted trailing spaces are dropped.
    fn end_field(fields: &mut Vec<String>, field: &mut String, quoted: &mut bool, last: bool) {
        if last && !*quoted {
            let t = field.trim_end_matches(' ').len();
            field.truncate(t);
        }
        fields.push(std::mem::take(field));
        *quoted = false;
    }

    while let Some(c) = chars.next() {
        if in_quotes {
            match c {
                '"' if chars.peek() == Some(&'"') => {
                    chars.next();
                    field.push('"');
                }
                '"' => in_quotes = false,
                '\n' => {
                    line += 1;
                    field.push('\n');
                }
                _ => field.push(c),
            }
            continue;
        }
        match c {
            '"' if field.is_empty() && !quoted => {
                in_quotes = true;
                quoted = true;
            }
            '\r' | '\n' => {
                if c == '\r' && chars.peek() == Some(&'\n') {
                    chars.next();
                }
                end_field(&mut fields, &mut field, &mut quoted, true);
                let rec = std::mem::take(&mut fields);
                if !(rec.len() == 1 && rec[0].is_empty()) {
                    out.push((start_line, rec));
                    if max.is_some_and(|m| out.len() >= m) {
                        return Ok(out);
                    }
                }
                line += 1;
                start_line = line;
            }
            c if c == delim => end_field(&mut fields, &mut field, &mut quoted, false),
            _ => field.push(c),
        }
    }
    if in_quotes {
        return Err(SavError::Csv(format!("baris {start_line}: tanda kutip tidak ditutup")));
    }
    if !field.is_empty() || !fields.is_empty() || quoted {
        end_field(&mut fields, &mut field, &mut quoted, true);
        if !(fields.len() == 1 && fields[0].is_empty()) {
            out.push((start_line, fields));
        }
    }
    Ok(out)
}

/// `\0` means a single column: no delimiter at all.
fn detect_delimiter(text: &str) -> char {
    const CANDIDATES: [char; 3] = [',', ';', '\t'];
    let mut best: Option<(usize, char)> = None;
    let mut fallback: (usize, char) = (1, '\0');
    for d in CANDIDATES {
        let Ok(recs) = parse_records(text, d, Some(20)) else { continue };
        let Some(first) = recs.first().map(|r| r.1.len()) else { continue };
        if first > fallback.0 {
            fallback = (first, d);
        }
        if first > 1 && recs.iter().all(|r| r.1.len() == first) && best.is_none_or(|(n, _)| first > n) {
            best = Some((first, d));
        }
    }
    best.map_or(fallback.1, |b| b.1)
}

fn decode(bytes: &[u8]) -> String {
    if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        return encoding_rs::UTF_16LE.decode_without_bom_handling(rest).0.into_owned();
    }
    if let Some(rest) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        return encoding_rs::UTF_16BE.decode_without_bom_handling(rest).0.into_owned();
    }
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        // Excel's "CSV" export in an ANSI code page.
        Err(_) => encoding_rs::WINDOWS_1252.decode_without_bom_handling(bytes).0.into_owned(),
    }
}

/// Import a CSV/TXT file. See [`CsvOptions`]; the delimiter is the most frequent and consistent one of
/// `,` `;` tab over the first 20 lines, and a `,` decimal separator is detected when the delimiter is not `,`.
pub fn import_csv(path: &Path, opt: &CsvOptions) -> Result<Dataset, SavError> {
    let text = decode(&std::fs::read(path)?);
    let delim = opt.delimiter.map_or_else(|| detect_delimiter(&text), char::from);
    let recs = parse_records(&text, delim, None)?;
    let Some((_, first)) = recs.first() else {
        return Err(SavError::Csv("file kosong".into()));
    };
    let width = first.len();
    for (line, r) in &recs {
        if r.len() != width {
            return Err(SavError::Csv(format!("baris {line}: {} kolom, seharusnya {width}", r.len())));
        }
    }
    let (headers, body): (Vec<String>, &[Record]) =
        if opt.header { (first.clone(), &recs[1..]) } else { (vec![String::new(); width], &recs[..]) };
    let decimal_comma = opt.decimal_comma.unwrap_or_else(|| {
        delim != ',' && body.iter().flat_map(|r| &r.1).any(|c| c.contains(',') && parse_number(c, true).is_some())
    });
    let mut cols: Vec<Vec<Cell>> = vec![Vec::with_capacity(body.len()); width];
    for (_, r) in body {
        for (col, cell) in cols.iter_mut().zip(r) {
            col.push(classify(cell, decimal_comma));
        }
    }
    build_dataset(&headers, cols)
}
