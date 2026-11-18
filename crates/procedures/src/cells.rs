//! Cell and header helpers shared by the procedures.
use dataset::{Dataset, VarId};
use output_model::{Cell, CellFormat, CellValue, HeaderNode, OutputItem};

pub(crate) fn label(ds: &Dataset, id: VarId) -> String {
    let v = ds.var(id);
    v.label.clone().filter(|l| !l.is_empty()).unwrap_or_else(|| v.name.clone())
}

pub(crate) fn leaf(s: &str) -> HeaderNode {
    HeaderNode { label: s.to_string(), children: vec![] }
}

pub(crate) fn group(s: &str, children: &[&str]) -> HeaderNode {
    HeaderNode { label: s.to_string(), children: children.iter().map(|c| leaf(c)).collect() }
}

/// Numeric cell; a non-finite (undefined) statistic is shown as "." like SPSS system-missing.
pub(crate) fn num(v: f64, format: CellFormat) -> Cell {
    if v.is_finite() { Cell { value: CellValue::Num(v), format, footnotes: vec![] } } else { text(".") }
}

/// Numeric cell, or an empty cell when the value is NaN (statistic not applicable).
pub(crate) fn num_or_empty(v: f64, format: CellFormat) -> Cell {
    if v.is_nan() { empty() } else { num(v, format) }
}

pub(crate) fn text(s: &str) -> Cell {
    Cell { value: CellValue::Text(s.to_string()), format: CellFormat::Text, footnotes: vec![] }
}

pub(crate) fn empty() -> Cell {
    Cell { value: CellValue::Empty, format: CellFormat::Text, footnotes: vec![] }
}

pub(crate) fn foot(mut c: Cell, mark: char) -> Cell {
    c.footnotes.push(mark);
    c
}

pub(crate) fn warning(msg: impl Into<String>) -> Vec<OutputItem> {
    vec![OutputItem::Warning(msg.into())]
}

pub(crate) const LOWER_BOUND_NOTE: &str = "This is a lower bound of the true significance.";

/// `Fixed(decimals + extra)`, capped at 16 decimals.
pub(crate) fn fixed_plus(decimals: usize, extra: usize) -> CellFormat {
    CellFormat::Fixed((decimals + extra).min(16) as u8)
}
