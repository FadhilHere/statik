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

/// Valid, unique (case-insensitive) variable names for the header texts, in order.
fn variable_names(headers: &[String]) -> Vec<String> {
    let mut used: HashSet<String> = HashSet::new();
    headers
        .iter()
        .map(|raw| {
            let name = sanitize_name(raw, &|c: &str| used.contains(&c.to_lowercase()));
            used.insert(name.to_lowercase());
            name
        })
        .collect()
}

/// Build a dataset from header texts and columns of cells. A column is numeric when none of its cells is text.
pub(crate) fn build_dataset(headers: &[String], cols: Vec<Vec<Cell>>) -> Result<Dataset, SavError> {
    let mut ds = Dataset::new();
    for ((raw, name), cells) in headers.iter().zip(variable_names(headers)).zip(cols) {
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
        return Err(SavError::Csv(format!("line {start_line}: a quoted field is not closed")));
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

/// Decide the decimal separator when the options leave it open.
///
/// - `,` delimiter: decimal point.
/// - `;` or tab delimiter (Indonesian / European Excel exports): decimal comma, with `.` as the
///   thousands separator, so `750.000` is 750000. Only when some cell makes sense with a decimal
///   point but not with a decimal comma (`78.5`, `3.14159`) and no cell needs the decimal comma
///   (`78,5`, `1.234,5`) does the file use decimal points.
/// - No delimiter (one column): decimal comma only if some cell needs it (`78,5`).
fn detect_decimal_comma<'a>(delim: char, mut cells: impl Iterator<Item = &'a String>) -> bool {
    match delim {
        ',' => false,
        ';' | '\t' => {
            let (mut comma_only, mut dot_only) = (false, false);
            for c in cells {
                let dot = parse_number(c, false).is_some();
                let comma = parse_number(c, true).is_some();
                comma_only |= comma && !dot;
                dot_only |= dot && !comma;
            }
            comma_only || !dot_only
        }
        _ => cells.any(|c| c.contains(',') && parse_number(c, true).is_some()),
    }
}

/// A CSV file split into records with the delimiter and decimal separator decided.
struct Parsed {
    delim: char,
    decimal_comma: bool,
    headers: Vec<String>,
    body: Vec<Record>,
}

fn parse_file(path: &Path, opt: &CsvOptions) -> Result<Parsed, SavError> {
    parse_text(&decode(&std::fs::read(path)?), opt)
}

fn parse_text(text: &str, opt: &CsvOptions) -> Result<Parsed, SavError> {
    let delim = opt.delimiter.map_or_else(|| detect_delimiter(text), char::from);
    let mut recs = parse_records(text, delim, None)?;
    let Some((_, first)) = recs.first() else {
        return Err(SavError::Csv("the file is empty".into()));
    };
    let width = first.len();
    for (line, r) in &recs {
        if r.len() != width {
            let fields = if r.len() == 1 { "field" } else { "fields" };
            return Err(SavError::Csv(format!("line {line} has {} {fields}, expected {width}", r.len())));
        }
    }
    let headers = if opt.header { recs.remove(0).1 } else { vec![String::new(); width] };
    let decimal_comma =
        opt.decimal_comma.unwrap_or_else(|| detect_decimal_comma(delim, recs.iter().flat_map(|r| &r.1)));
    Ok(Parsed { delim, decimal_comma, headers, body: recs })
}

/// Import a CSV/TXT file. See [`CsvOptions`]; the delimiter is the most frequent and consistent one of
/// `,` `;` tab over the first 20 lines. The decimal separator rule is described in KNOWN_GAPS.md
/// ("CSV / XLSX import"): with `;` or tab a `.` is a thousands separator unless the file shows otherwise.
pub fn import_csv(path: &Path, opt: &CsvOptions) -> Result<Dataset, SavError> {
    let p = parse_file(path, opt)?;
    let mut cols: Vec<Vec<Cell>> = vec![Vec::with_capacity(p.body.len()); p.headers.len()];
    for (_, r) in &p.body {
        for (col, cell) in cols.iter_mut().zip(r) {
            col.push(classify(cell, p.decimal_comma));
        }
    }
    build_dataset(&p.headers, cols)
}

/// What [`import_csv`] would do with a file, for an import dialog's preview.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CsvPreview {
    /// The delimiter used (detected or given); `0` when the file has a single column.
    pub delimiter: u8,
    /// Whether `,` is the decimal separator (detected or given).
    pub decimal_comma: bool,
    /// The variable names the import would create (sanitized, unique).
    pub header: Vec<String>,
    /// The header cells as in the file (empty strings when `header` is off).
    pub raw_header: Vec<String>,
    /// The first data rows (at most `max_rows`), as raw cell text.
    pub rows: Vec<Vec<String>>,
}

/// Bytes of a file [`sniff_csv`] looks at.
pub const SNIFF_BYTES: usize = 1 << 20;

/// The first `max_bytes` bytes of a CSV file, decoded like [`import_csv`] does and cut after the last
/// complete record (never inside a quoted field or a character). A file no longer than `max_bytes` is
/// returned whole. Used for previews, so a huge file is never read completely on the UI thread.
pub fn csv_prefix(path: &Path, max_bytes: usize) -> Result<String, SavError> {
    use std::io::Read;
    let mut buf = Vec::with_capacity(max_bytes.min(1 << 24) + 1);
    std::fs::File::open(path)?.take(max_bytes as u64 + 1).read_to_end(&mut buf)?;
    if buf.len() <= max_bytes {
        return Ok(decode(&buf));
    }
    buf.truncate(max_bytes);
    let mut text = decode_prefix(&buf);
    let end = last_record_end(&text);
    if end > 0 {
        text.truncate(end);
    }
    Ok(text)
}

/// Decode a cut-off byte prefix: an incomplete character at the end is dropped.
fn decode_prefix(bytes: &[u8]) -> String {
    let utf16 = bytes.starts_with(&[0xFF, 0xFE]) || bytes.starts_with(&[0xFE, 0xFF]);
    if utf16 {
        let n = bytes.len() & !1;
        return decode(&bytes[..n]).trim_end_matches('\u{FFFD}').to_string();
    }
    let body = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    match std::str::from_utf8(body) {
        Err(e) if e.error_len().is_none() => decode(&bytes[..bytes.len() - (body.len() - e.valid_up_to())]),
        _ => decode(bytes),
    }
}

/// Byte offset just after the last line break that ends a record (outside quotes). A quote opens a
/// quoted field only at the start of a field; `,` `;` and tab all count as field separators here.
fn last_record_end(text: &str) -> usize {
    let mut end = 0;
    let (mut in_quotes, mut field_start) = (false, true);
    let mut it = text.char_indices().peekable();
    while let Some((i, c)) = it.next() {
        if in_quotes {
            if c == '"' {
                if it.peek().is_some_and(|&(_, n)| n == '"') {
                    it.next();
                } else {
                    in_quotes = false;
                }
            }
            continue;
        }
        match c {
            '"' if field_start => {
                in_quotes = true;
                field_start = false;
            }
            '\n' => {
                end = i + 1;
                field_start = true;
            }
            ',' | ';' | '\t' | '\r' => field_start = true,
            _ => field_start = false,
        }
    }
    end
}

/// Parse a CSV file like [`import_csv`] and report the detected settings, the variable names and the
/// first `max_rows` data rows. Only the first [`SNIFF_BYTES`] of the file are read ([`csv_prefix`]), so
/// detection is based on them and an error later in the file is only found by [`import_csv`].
pub fn sniff_csv(path: &Path, opt: &CsvOptions, max_rows: usize) -> Result<CsvPreview, SavError> {
    sniff_csv_text(&csv_prefix(path, SNIFF_BYTES)?, opt, max_rows)
}

/// [`sniff_csv`] on text already read (e.g. a [`csv_prefix`] kept by an import dialog, re-parsed
/// whenever an option changes).
pub fn sniff_csv_text(text: &str, opt: &CsvOptions, max_rows: usize) -> Result<CsvPreview, SavError> {
    let p = parse_text(text, opt)?;
    Ok(CsvPreview {
        delimiter: u8::try_from(p.delim).unwrap_or(0),
        decimal_comma: p.decimal_comma,
        header: variable_names(&p.headers),
        raw_header: p.headers,
        rows: p.body.into_iter().take(max_rows).map(|r| r.1).collect(),
    })
}
