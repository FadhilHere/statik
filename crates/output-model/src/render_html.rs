use crate::format::{format_markers, format_value, join_markers};
use crate::table::*;

fn sup(marks: &[char]) -> String {
    if marks.is_empty() { String::new() } else { format!("<sup>{}</sup>", esc(&join_markers(marks))) }
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

const TD: &str = "padding:2px 8px;border:1px solid #888;";
const FONT: &str = "font-family:Arial,sans-serif";

pub fn render_html(items: &[OutputItem]) -> String {
    let mut o = String::new();
    for it in items {
        match it {
            OutputItem::Title(t) => o.push_str(&format!("<h2 style=\"{FONT}\">{}</h2>\n", esc(t))),
            OutputItem::Notes(t) => o.push_str(&format!("<p style=\"{FONT}\">{}</p>\n", esc(t))),
            OutputItem::Warning(t) => {
                o.push_str(&format!("<p style=\"{FONT};color:#a00\">Warning: {}</p>\n", esc(t)))
            }
            OutputItem::Chart(c) => {
                let sub = c.subtitle().map(|s| format!(" ({})", esc(s))).unwrap_or_default();
                o.push_str(&format!("<p style=\"{FONT}\">[Chart: {}{sub}]</p>\n", esc(c.title())))
            }
            OutputItem::Table(t) => table(&mut o, t),
        }
    }
    o
}

fn table(o: &mut String, t: &Table) {
    let paths = leaf_paths(&t.row_headers);
    let rdepth = paths.iter().map(|p| p.len()).max().unwrap_or(1).max(1);
    o.push_str(&format!("<table style=\"border-collapse:collapse;{FONT};font-size:10pt\">\n"));
    o.push_str(&format!("<caption style=\"font-weight:bold;text-align:left\">{}{}</caption>\n", esc(&t.title), sup(&t.title_footnotes)));
    o.push_str("<thead>\n");
    let hcells = header_cells(&t.col_headers);
    let levels = header_depth(&t.col_headers);
    for lvl in 0..levels {
        o.push_str("<tr>");
        if lvl == 0 {
            o.push_str(&format!("<th colspan=\"{rdepth}\" rowspan=\"{levels}\" style=\"{TD}\"></th>"));
        }
        for h in hcells.iter().filter(|h| h.level == lvl) {
            o.push_str(&format!(
                "<th colspan=\"{}\" rowspan=\"{}\" style=\"{TD}text-align:center\">{}{}</th>",
                h.colspan,
                h.rowspan,
                esc(&h.label),
                sup(&h.footnotes)
            ));
        }
        o.push_str("</tr>\n");
    }
    o.push_str("</thead>\n<tbody>\n");
    for (ri, row) in t.cells.iter().enumerate() {
        o.push_str("<tr>");
        for d in 0..rdepth {
            let (l, f) = row_label(&paths, ri, d);
            o.push_str(&format!("<th style=\"{TD}text-align:left\">{}{}</th>", esc(&l), sup(&f)));
        }
        for c in row {
            let align = if matches!(c.value, CellValue::Num(_)) { "right" } else { "left" };
            let marks = format_markers(c);
            let marks = if marks.is_empty() { String::new() } else { format!("<sup>{}</sup>", esc(&marks)) };
            o.push_str(&format!("<td style=\"{TD}text-align:{align}\">{}{marks}</td>", esc(&format_value(c))));
        }
        o.push_str("</tr>\n");
    }
    o.push_str("</tbody>\n");
    if !t.footnotes.is_empty() {
        let span = rdepth + leaf_count(&t.col_headers).max(t.cells.iter().map(|r| r.len()).max().unwrap_or(0));
        o.push_str("<tfoot>\n");
        for (m, txt) in &t.footnotes {
            o.push_str(&format!(
                "<tr><td colspan=\"{span}\" style=\"font-size:9pt;text-align:left\">{}. {}</td></tr>\n",
                esc(&m.to_string()),
                esc(txt)
            ));
        }
        o.push_str("</tfoot>\n");
    }
    o.push_str("</table>\n");
}

// Word-ready table (clipboard): SPSS pivot look with inline CSS only (Word ignores <style>
// blocks in pasted fragments). Dark rules above the header and below the last row, a light
// rule under the header and between rows, labels in SPSS's slate blue, numbers right-aligned.
const RULE: &str = "1px solid #152935";
const LIGHT: &str = "1px solid #AEAEAE";
const FAINT: &str = "1px solid #E0E0E0";
const CELL: &str = "padding:1pt 5pt;font-family:Arial,sans-serif;font-size:9pt;";
const LABEL_COLOR: &str = "color:#264A60;";

/// One table as a self-contained, inline-styled HTML `<table>` (for the clipboard "HTML Format"):
/// the title is the first row (centred, bold), header labels span their children, row group
/// labels span their rows, footnotes are small rows at the bottom. Footnote markers are `<sup>`.
pub fn render_table_html(t: &Table) -> String {
    let (rdepth, rcells) = row_header_cells(&t.row_headers);
    let ncols = leaf_count(&t.col_headers).max(t.cells.iter().map(|r| r.len()).max().unwrap_or(0));
    let total = rdepth + ncols;
    let levels = header_depth(&t.col_headers);
    let nrows = t.cells.len();
    let mut o = String::new();
    o.push_str("<table style=\"border-collapse:collapse;border-spacing:0;font-family:Arial,sans-serif;font-size:9pt\">\n");
    o.push_str(&format!(
        "<tr><td colspan=\"{total}\" style=\"{CELL}font-weight:bold;text-align:center;padding-bottom:4pt;border:none\">{}{}</td></tr>\n",
        esc(&t.title),
        sup(&t.title_footnotes)
    ));
    let hcells = header_cells(&t.col_headers);
    for lvl in 0..levels {
        o.push_str("<tr>");
        if lvl == 0 {
            o.push_str(&format!(
                "<td colspan=\"{rdepth}\" rowspan=\"{levels}\" style=\"{CELL}border-top:{RULE};border-bottom:{LIGHT}\"></td>"
            ));
        }
        for h in hcells.iter().filter(|h| h.level == lvl) {
            let top = if lvl == 0 { format!("border-top:{RULE};") } else { String::new() };
            o.push_str(&format!(
                "<td colspan=\"{}\" rowspan=\"{}\" style=\"{CELL}{LABEL_COLOR}{top}border-bottom:{LIGHT};text-align:center;vertical-align:bottom\">{}{}</td>",
                h.colspan,
                h.rowspan,
                esc(&h.label),
                sup(&h.footnotes)
            ));
        }
        o.push_str("</tr>\n");
    }
    for (ri, row) in t.cells.iter().enumerate() {
        let last = ri + 1 == nrows;
        let bottom = if last { RULE } else { FAINT };
        let top = if levels == 0 && ri == 0 { format!("border-top:{RULE};") } else { String::new() };
        o.push_str("<tr>");
        for h in rcells.iter().filter(|h| h.row == ri) {
            let b = if h.row + h.rowspan == nrows { RULE } else { FAINT };
            o.push_str(&format!(
                "<td colspan=\"{}\" rowspan=\"{}\" style=\"{CELL}{LABEL_COLOR}{top}border-bottom:{b};text-align:left;vertical-align:top\">{}{}</td>",
                h.colspan,
                h.rowspan,
                esc(&h.label),
                sup(&h.footnotes)
            ));
        }
        if t.row_headers.is_empty() {
            o.push_str(&format!("<td style=\"{CELL}{top}border-bottom:{bottom}\"></td>"));
        }
        for (ci, c) in row.iter().enumerate() {
            let align = if matches!(c.value, CellValue::Num(_)) { "right" } else { "left" };
            let left = if ci == 0 { format!("border-left:{LIGHT};") } else { String::new() };
            o.push_str(&format!(
                "<td style=\"{CELL}{top}{left}border-bottom:{bottom};text-align:{align}\">{}{}</td>",
                esc(&format_value(c)),
                sup(&c.footnotes)
            ));
        }
        for _ in row.len()..ncols {
            o.push_str(&format!("<td style=\"{CELL}{top}border-bottom:{bottom}\"></td>"));
        }
        o.push_str("</tr>\n");
    }
    for (m, txt) in &t.footnotes {
        o.push_str(&format!(
            "<tr><td colspan=\"{total}\" style=\"{CELL}font-size:8pt;text-align:left;border:none\">{}. {}</td></tr>\n",
            esc(&m.to_string()),
            esc(txt)
        ));
    }
    o.push_str("</table>\n");
    o
}
