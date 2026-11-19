use output_model::*;

fn hn(s: &str, children: Vec<HeaderNode>) -> HeaderNode {
    HeaderNode { label: s.into(), footnotes: vec![], children }
}

fn num(v: f64) -> Cell {
    Cell { value: CellValue::Num(v), format: CellFormat::Fixed(3), footnotes: vec![] }
}

/// ANOVA-like: Model "1" over Regression/Residual/Total, plus a short path (spans both columns).
fn anova() -> Table {
    Table {
        title: "ANOVA".into(),
        title_footnotes: vec!['a'],
        col_headers: vec![hn("Sum of Squares", vec![]), hn("Sig.", vec![])],
        row_headers: vec![
            hn("1", vec![hn("Regression", vec![]), hn("Residual", vec![]), hn("Total", vec![])]),
            HeaderNode { label: "All".into(), footnotes: vec!['b'], children: vec![] },
        ],
        cells: vec![vec![num(1.0), num(0.5)], vec![num(2.0), num(0.25)], vec![num(3.0), num(0.0)], vec![num(4.0), num(1.0)]],
        footnotes: vec![('a', "Dependent Variable: y".into()), ('b', "x < 1 & \"q\"".into())],
    }
}

#[test]
fn row_header_cells_span_groups_and_short_paths() {
    let (depth, cells) = row_header_cells(&anova().row_headers);
    assert_eq!(depth, 2);
    let find = |l: &str| cells.iter().find(|c| c.label == l).unwrap();
    let one = find("1");
    assert_eq!((one.row, one.depth, one.rowspan, one.colspan), (0, 0, 3, 1));
    let res = find("Residual");
    assert_eq!((res.row, res.depth, res.rowspan, res.colspan), (1, 1, 1, 1));
    let all = find("All");
    assert_eq!((all.row, all.depth, all.rowspan, all.colspan), (3, 0, 1, 2));
    assert_eq!(all.footnotes, vec!['b']);
    assert_eq!(cells.len(), 5);
}

#[test]
fn table_html_is_inline_styled_with_markers() {
    let h = render_table_html(&anova());
    assert!(h.starts_with("<table"), "{h}");
    assert!(h.trim_end().ends_with("</table>"), "{h}");
    assert!(h.contains("ANOVA<sup>a</sup>"), "{h}");
    assert!(h.contains("rowspan=\"3\""), "{h}");
    assert!(h.contains(">.500</td>"), "{h}");
    assert!(h.contains("x &lt; 1 &amp; &quot;q&quot;"), "{h}");
    assert!(h.contains("text-align:right"), "{h}");
    assert!(!h.contains("class="), "no external CSS: {h}");
    assert!(!h.contains("<style"), "no style block: {h}");
}
