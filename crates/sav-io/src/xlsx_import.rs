//! XLSX/XLS/ODS import through `calamine`.
use crate::SavError;
use crate::csv_import::{Cell, build_dataset};
use calamine::{Data, Reader, open_workbook_auto};
use dataset::Dataset;
use std::path::Path;

/// A float as text the way a spreadsheet shows it: whole numbers without `.0`.
fn float_text(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 { format!("{}", v as i64) } else { format!("{v}") }
}

fn decimals_of(v: f64) -> usize {
    format!("{v}").split_once('.').map_or(0, |(_, f)| f.len())
}

fn num_cell(v: f64) -> Cell {
    if v.is_finite() { Cell::Num(v + 0.0, decimals_of(v), float_text(v)) } else { Cell::Empty }
}

fn cell(d: &Data) -> Cell {
    match d {
        Data::Empty | Data::Error(_) => Cell::Empty,
        Data::String(s) if s.trim().is_empty() => Cell::Empty,
        Data::String(s) => Cell::Text(s.clone()),
        Data::Float(v) => num_cell(*v),
        Data::Int(i) => num_cell(*i as f64),
        Data::Bool(b) => Cell::Text(if *b { "TRUE" } else { "FALSE" }.into()),
        // Dates become Excel serial numbers (see KNOWN_GAPS.md).
        Data::DateTime(dt) => num_cell(dt.as_f64()),
        Data::DateTimeIso(s) | Data::DurationIso(s) => Cell::Text(s.clone()),
    }
}

fn header_text(d: &Data) -> String {
    match d {
        Data::Empty | Data::Error(_) => String::new(),
        Data::String(s) => s.clone(),
        Data::Float(v) => float_text(*v),
        Data::Int(i) => i.to_string(),
        Data::Bool(b) => if *b { "TRUE" } else { "FALSE" }.into(),
        Data::DateTime(dt) => float_text(dt.as_f64()),
        Data::DateTimeIso(s) | Data::DurationIso(s) => s.clone(),
    }
}

/// Import one worksheet (the first one by default). A column holding both numbers and text becomes a
/// string variable; empty cells are system-missing (numeric) or empty (string).
pub fn import_xlsx(path: &Path, sheet: Option<&str>, header: bool) -> Result<Dataset, SavError> {
    std::fs::File::open(path)?; // a missing file is an Io error, not a format error
    let mut wb = open_workbook_auto(path).map_err(|e| SavError::Format(format!("tidak bisa membuka {}: {e}", path.display())))?;
    let name = match sheet {
        Some(s) => s.to_string(),
        None => wb.sheet_names().first().cloned().ok_or_else(|| SavError::Format("workbook tanpa sheet".into()))?,
    };
    if !wb.sheet_names().contains(&name) {
        return Err(SavError::Format(format!("sheet '{name}' tidak ditemukan")));
    }
    let range = wb.worksheet_range(&name).map_err(|e| SavError::Format(format!("sheet '{name}': {e}")))?;
    let width = range.width();
    if width == 0 {
        return Err(SavError::Format(format!("sheet '{name}' kosong")));
    }
    let mut rows = range.rows();
    let headers: Vec<String> = if header {
        rows.next().map(|r| r.iter().map(header_text).collect()).unwrap_or_default()
    } else {
        vec![String::new(); width]
    };
    let mut cols: Vec<Vec<Cell>> = vec![Vec::new(); width];
    for r in rows {
        for (col, d) in cols.iter_mut().zip(r) {
            col.push(cell(d));
        }
    }
    build_dataset(&headers, cols)
}
