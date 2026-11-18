//! Cell and header helpers shared by the procedures.
use dataset::{Dataset, VarId};
use output_model::{Cell, CellFormat, CellValue, HeaderNode, OutputItem};

pub(crate) fn label(ds: &Dataset, id: VarId) -> String {
    let v = ds.var(id);
    v.label.clone().filter(|l| !l.is_empty()).unwrap_or_else(|| v.name.clone())
}

pub(crate) fn leaf(s: &str) -> HeaderNode {
    HeaderNode { label: s.to_string(), footnotes: vec![], children: vec![] }
}

pub(crate) fn group(s: &str, children: &[&str]) -> HeaderNode {
    HeaderNode { label: s.to_string(), footnotes: vec![], children: children.iter().map(|c| leaf(c)).collect() }
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

/// Attach footnote markers to a header node (SPSS superscript after the label).
pub(crate) fn marked(mut n: HeaderNode, marks: &[char]) -> HeaderNode {
    n.footnotes.extend_from_slice(marks);
    n
}

/// Header node with arbitrary child nodes.
pub(crate) fn parent(s: &str, children: Vec<HeaderNode>) -> HeaderNode {
    HeaderNode { label: s.to_string(), footnotes: vec![], children }
}

/// "Significance" header with One-Sided p / Two-Sided p sub-columns (SPSS 27+).
pub(crate) fn sig_header() -> HeaderNode {
    group("Significance", &["One-Sided p", "Two-Sided p"])
}

/// df cell: a count for integer df, three decimals otherwise.
pub(crate) fn df_cell(df: f64) -> Cell {
    if df.fract() == 0.0 { num(df, CellFormat::Count) } else { num(df, CellFormat::Fixed(3)) }
}

pub(crate) const SD_ZERO_MSG: &str = "t cannot be computed because the standard deviation is 0.";

/// Stale-id guard: `VarId` is positional, so after structural edits an id may be out of range.
/// Returns a single Warning item for the first missing variable.
pub(crate) fn check_ids(ds: &Dataset, ids: &[VarId], proc_name: &str) -> Result<(), Vec<OutputItem>> {
    match ids.iter().find(|id| ds.try_var(**id).is_none()) {
        Some(id) => Err(warning(format!("{proc_name}: variable no longer exists (index {}).", id.0))),
        None => Ok(()),
    }
}

/// Like [`check_ids`], and additionally every id must be a numeric variable.
pub(crate) fn check_numeric_ids(ds: &Dataset, ids: &[VarId], proc_name: &str) -> Result<(), Vec<OutputItem>> {
    check_ids(ds, ids, proc_name)?;
    match ids.iter().find(|id| !matches!(ds.var(**id).var_type, dataset::VarType::Numeric)) {
        Some(id) => Err(warning(format!("{proc_name}: variable {} is not numeric.", ds.var(*id).name))),
        None => Ok(()),
    }
}
