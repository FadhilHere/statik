use crate::format::format_cell;
use crate::table::*;

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
            OutputItem::Chart(c) => o.push_str(&format!("<p style=\"{FONT}\">[Chart: {}]</p>\n", esc(c.title()))),
            OutputItem::Table(t) => table(&mut o, t),
        }
    }
    o
}

fn table(o: &mut String, t: &Table) {
    let paths = leaf_paths(&t.row_headers);
    let rdepth = paths.iter().map(|p| p.len()).max().unwrap_or(1).max(1);
    o.push_str(&format!("<table style=\"border-collapse:collapse;{FONT};font-size:10pt\">\n"));
    o.push_str(&format!("<caption style=\"font-weight:bold;text-align:left\">{}</caption>\n", esc(&t.title)));
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
                "<th colspan=\"{}\" rowspan=\"{}\" style=\"{TD}text-align:center\">{}</th>",
                h.colspan,
                h.rowspan,
                esc(&h.label)
            ));
        }
        o.push_str("</tr>\n");
    }
    o.push_str("</thead>\n<tbody>\n");
    for (ri, row) in t.cells.iter().enumerate() {
        o.push_str("<tr>");
        for d in 0..rdepth {
            o.push_str(&format!("<th style=\"{TD}text-align:left\">{}</th>", esc(&row_label(&paths, ri, d))));
        }
        for c in row {
            let align = if matches!(c.value, CellValue::Num(_)) { "right" } else { "left" };
            o.push_str(&format!("<td style=\"{TD}text-align:{align}\">{}</td>", esc(&format_cell(c))));
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
