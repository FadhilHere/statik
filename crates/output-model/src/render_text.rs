use crate::format::{format_cell, join_markers};
use crate::table::*;

pub fn render_text(items: &[OutputItem]) -> String {
    let mut o = String::new();
    for it in items {
        match it {
            OutputItem::Title(t) | OutputItem::Notes(t) => o.push_str(&format!("{t}\n\n")),
            OutputItem::Warning(t) => o.push_str(&format!("Warning: {t}\n\n")),
            OutputItem::Chart(c) => match c.subtitle() {
                Some(s) => o.push_str(&format!("[Chart: {} ({s})]\n\n", c.title())),
                None => o.push_str(&format!("[Chart: {}]\n\n", c.title())),
            },
            OutputItem::Table(t) => table(&mut o, t),
        }
    }
    o
}

fn table(o: &mut String, t: &Table) {
    let paths = leaf_paths(&t.row_headers);
    let rdepth = paths.iter().map(|p| p.len()).max().unwrap_or(1).max(1);
    let ncols = leaf_count(&t.col_headers).max(t.cells.iter().map(|r| r.len()).max().unwrap_or(0));
    let levels = header_depth(&t.col_headers);
    let total = rdepth + ncols;

    let mut grid: Vec<Vec<String>> = vec![vec![String::new(); total]; levels];
    for h in header_cells(&t.col_headers) {
        // Leaf headers sit on the bottom header row so they line up with the data.
        let row = if h.rowspan > 1 { h.level + h.rowspan - 1 } else { h.level };
        grid[row][rdepth + h.col] = format!("{}{}", h.label, join_markers(&h.footnotes));
    }
    for (ri, row) in t.cells.iter().enumerate() {
        let mut line = vec![String::new(); total];
        for (d, slot) in line.iter_mut().enumerate().take(rdepth) {
            let (l, f) = row_label(&paths, ri, d);
            *slot = format!("{l}{}", join_markers(&f));
        }
        for (ci, c) in row.iter().enumerate() {
            line[rdepth + ci] = format_cell(c);
        }
        grid.push(line);
    }

    let mut w = vec![0usize; total];
    for r in &grid {
        for (i, c) in r.iter().enumerate() {
            w[i] = w[i].max(c.chars().count());
        }
    }
    let line_w = w.iter().sum::<usize>() + 2 * total.saturating_sub(1);
    let rule = "-".repeat(line_w);
    o.push_str(&format!("{}{}\n{rule}\n", t.title, join_markers(&t.title_footnotes)));
    for (ri, r) in grid.iter().enumerate() {
        let parts: Vec<String> = r
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let pad = " ".repeat(w[i] - c.chars().count());
                if i < rdepth { format!("{c}{pad}") } else { format!("{pad}{c}") }
            })
            .collect();
        o.push_str(parts.join("  ").trim_end());
        o.push('\n');
        if levels > 0 && ri + 1 == levels {
            o.push_str(&rule);
            o.push('\n');
        }
    }
    o.push_str(&rule);
    o.push('\n');
    for (m, txt) in &t.footnotes {
        o.push_str(&format!("{m}. {txt}\n"));
    }
    o.push('\n');
}
