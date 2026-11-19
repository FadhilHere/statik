use crate::chart::Chart;
use serde::{Deserialize, Serialize};

/// serde adapter for `f64`: NaN and the infinities (which JSON cannot hold) are written as the
/// strings "NaN", "inf" and "-inf".
mod num_f64 {
    use serde::de::{self, Visitor};
    use serde::{Deserializer, Serializer};

    pub fn serialize<S: Serializer>(v: &f64, s: S) -> Result<S::Ok, S::Error> {
        if v.is_finite() {
            s.serialize_f64(*v)
        } else if v.is_nan() {
            s.serialize_str("NaN")
        } else if *v > 0.0 {
            s.serialize_str("inf")
        } else {
            s.serialize_str("-inf")
        }
    }

    struct V;
    impl Visitor<'_> for V {
        type Value = f64;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("a number or one of \"NaN\", \"inf\", \"-inf\"")
        }
        fn visit_f64<E: de::Error>(self, v: f64) -> Result<f64, E> {
            Ok(v)
        }
        fn visit_i64<E: de::Error>(self, v: i64) -> Result<f64, E> {
            Ok(v as f64)
        }
        fn visit_u64<E: de::Error>(self, v: u64) -> Result<f64, E> {
            Ok(v as f64)
        }
        fn visit_str<E: de::Error>(self, v: &str) -> Result<f64, E> {
            match v {
                "NaN" => Ok(f64::NAN),
                "inf" => Ok(f64::INFINITY),
                "-inf" => Ok(f64::NEG_INFINITY),
                _ => Err(E::invalid_value(de::Unexpected::Str(v), &self)),
            }
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<f64, D::Error> {
        d.deserialize_any(V)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum CellFormat {
    Fixed(u8),
    Sig,
    Count,
    Text,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CellValue {
    Num(#[serde(with = "num_f64")] f64),
    Text(String),
    Empty,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Cell {
    pub value: CellValue,
    pub format: CellFormat,
    pub footnotes: Vec<char>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HeaderNode {
    pub label: String,
    /// Footnote markers shown after the label (SPSS superscripts on headers).
    pub footnotes: Vec<char>,
    pub children: Vec<HeaderNode>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Table {
    pub title: String,
    /// Footnote markers shown after the title (e.g. "Coefficients^a").
    pub title_footnotes: Vec<char>,
    pub col_headers: Vec<HeaderNode>,
    pub row_headers: Vec<HeaderNode>,
    pub cells: Vec<Vec<Cell>>,
    pub footnotes: Vec<(char, String)>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum OutputItem {
    Title(String),
    Notes(String),
    Table(Table),
    Chart(Chart),
    Warning(String),
}

fn leaves(n: &HeaderNode) -> usize {
    if n.children.is_empty() {
        1
    } else {
        n.children.iter().map(leaves).sum()
    }
}

fn depth(nodes: &[HeaderNode]) -> usize {
    nodes.iter().map(|n| 1 + depth(&n.children)).max().unwrap_or(0)
}

/// Number of leaf columns/rows.
pub fn leaf_count(nodes: &[HeaderNode]) -> usize {
    nodes.iter().map(leaves).sum()
}

/// One header cell placed on a grid: level (row), starting leaf column, spans.
pub struct HeaderCell {
    pub label: String,
    pub footnotes: Vec<char>,
    pub level: usize,
    pub col: usize,
    pub colspan: usize,
    pub rowspan: usize,
}

/// Total number of header levels.
pub fn header_depth(nodes: &[HeaderNode]) -> usize {
    depth(nodes)
}

/// Lay header nodes out on a grid (leaves span down to the bottom level).
pub fn header_cells(nodes: &[HeaderNode]) -> Vec<HeaderCell> {
    fn walk(n: &HeaderNode, level: usize, col: usize, total: usize, out: &mut Vec<HeaderCell>) {
        let rowspan = if n.children.is_empty() { total - level } else { 1 };
        out.push(HeaderCell { label: n.label.clone(), footnotes: n.footnotes.clone(), level, col, colspan: leaves(n), rowspan });
        let mut c = col;
        for ch in &n.children {
            walk(ch, level + 1, c, total, out);
            c += leaves(ch);
        }
    }
    let total = depth(nodes);
    let mut out = vec![];
    let mut col = 0;
    for n in nodes {
        walk(n, 0, col, total, &mut out);
        col += leaves(n);
    }
    out
}

/// A header label with its footnote markers.
pub type Label = (String, Vec<char>);

/// Path of labels (root..leaf) for each leaf.
pub fn leaf_paths(nodes: &[HeaderNode]) -> Vec<Vec<Label>> {
    fn walk(n: &HeaderNode, prefix: &mut Vec<Label>, out: &mut Vec<Vec<Label>>) {
        prefix.push((n.label.clone(), n.footnotes.clone()));
        if n.children.is_empty() {
            out.push(prefix.clone());
        } else {
            for c in &n.children {
                walk(c, prefix, out);
            }
        }
        prefix.pop();
    }
    let mut out = vec![];
    for n in nodes {
        walk(n, &mut vec![], &mut out);
    }
    out
}

/// Row-header label to show at depth `d` of row `ri` (blank when it repeats the row above).
pub fn row_label(paths: &[Vec<Label>], ri: usize, d: usize) -> Label {
    let Some(p) = paths.get(ri) else { return Label::default() };
    let Some(l) = p.get(d) else { return Label::default() };
    let repeat = d + 1 < p.len() && ri > 0 && paths[ri - 1].len() > d && paths[ri - 1][..=d] == p[..=d];
    if repeat { Label::default() } else { l.clone() }
}

/// One row-header cell placed on the row-header grid: first data row, depth (column), spans.
#[derive(Debug, Clone, PartialEq)]
pub struct RowHeaderCell {
    pub label: String,
    pub footnotes: Vec<char>,
    pub row: usize,
    pub depth: usize,
    /// Rows covered: a group label spans the rows of its children (SPSS "Model 1").
    pub rowspan: usize,
    /// Columns covered: the last label of a path shorter than the deepest one spans the rest.
    pub colspan: usize,
}

/// Lay the row headers out: returns the number of row-header columns (at least 1) and the
/// non-blank cells. A label repeated on the rows below (see [`row_label`]) spans them.
pub fn row_header_cells(nodes: &[HeaderNode]) -> (usize, Vec<RowHeaderCell>) {
    let paths = leaf_paths(nodes);
    let depth = paths.iter().map(|p| p.len()).max().unwrap_or(1).max(1);
    let mut out = vec![];
    for (ri, p) in paths.iter().enumerate() {
        for d in 0..p.len() {
            let (label, footnotes) = row_label(&paths, ri, d);
            let repeated = d + 1 < p.len() && ri > 0 && paths[ri - 1].len() > d && paths[ri - 1][..=d] == p[..=d];
            if repeated {
                continue;
            }
            let mut rowspan = 1;
            if d + 1 < p.len() {
                while let Some(q) = paths.get(ri + rowspan) {
                    if d + 1 < q.len() && q.len() > d && q[..=d] == p[..=d] {
                        rowspan += 1;
                    } else {
                        break;
                    }
                }
            }
            let colspan = if d + 1 == p.len() { depth - d } else { 1 };
            out.push(RowHeaderCell { label, footnotes, row: ri, depth: d, rowspan, colspan });
        }
    }
    (depth, out)
}
