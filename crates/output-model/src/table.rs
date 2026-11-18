use crate::chart::Chart;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CellFormat {
    Fixed(u8),
    Sig,
    Count,
    Text,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CellValue {
    Num(f64),
    Text(String),
    Empty,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Cell {
    pub value: CellValue,
    pub format: CellFormat,
    pub footnotes: Vec<char>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HeaderNode {
    pub label: String,
    pub children: Vec<HeaderNode>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Table {
    pub title: String,
    pub col_headers: Vec<HeaderNode>,
    pub row_headers: Vec<HeaderNode>,
    pub cells: Vec<Vec<Cell>>,
    pub footnotes: Vec<(char, String)>,
}

#[derive(Debug, Clone, PartialEq)]
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
pub(crate) fn leaf_count(nodes: &[HeaderNode]) -> usize {
    nodes.iter().map(leaves).sum()
}

/// One header cell placed on a grid: level (row), starting leaf column, spans.
pub(crate) struct HeaderCell {
    pub label: String,
    pub level: usize,
    pub col: usize,
    pub colspan: usize,
    pub rowspan: usize,
}

/// Total number of header levels.
pub(crate) fn header_depth(nodes: &[HeaderNode]) -> usize {
    depth(nodes)
}

/// Lay header nodes out on a grid (leaves span down to the bottom level).
pub(crate) fn header_cells(nodes: &[HeaderNode]) -> Vec<HeaderCell> {
    fn walk(n: &HeaderNode, level: usize, col: usize, total: usize, out: &mut Vec<HeaderCell>) {
        let rowspan = if n.children.is_empty() { total - level } else { 1 };
        out.push(HeaderCell { label: n.label.clone(), level, col, colspan: leaves(n), rowspan });
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

/// Path of labels (root..leaf) for each leaf.
pub(crate) fn leaf_paths(nodes: &[HeaderNode]) -> Vec<Vec<String>> {
    fn walk(n: &HeaderNode, prefix: &mut Vec<String>, out: &mut Vec<Vec<String>>) {
        prefix.push(n.label.clone());
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
pub(crate) fn row_label(paths: &[Vec<String>], ri: usize, d: usize) -> String {
    let Some(p) = paths.get(ri) else { return String::new() };
    let Some(l) = p.get(d) else { return String::new() };
    let repeat = d + 1 < p.len() && ri > 0 && paths[ri - 1].len() > d && paths[ri - 1][..=d] == p[..=d];
    if repeat { String::new() } else { l.clone() }
}
