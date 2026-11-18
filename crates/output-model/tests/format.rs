use output_model::*;

fn num(v: f64, f: CellFormat) -> Cell {
    Cell { value: CellValue::Num(v), format: f, footnotes: vec![] }
}

#[test]
fn fixed_drops_leading_zero() {
    assert_eq!(format_cell(&num(0.912, CellFormat::Fixed(3))), ".912");
    assert_eq!(format_cell(&num(-0.358, CellFormat::Fixed(3))), "-.358");
    assert_eq!(format_cell(&num(12.5, CellFormat::Fixed(3))), "12.500");
    assert_eq!(format_cell(&num(-0.0001, CellFormat::Fixed(3))), ".000");
    assert_eq!(format_cell(&num(-12.5, CellFormat::Fixed(1))), "-12.5");
    assert_eq!(format_cell(&num(0.5, CellFormat::Fixed(0))), "1");
}

#[test]
fn sig_rules() {
    assert_eq!(format_cell(&num(0.0001, CellFormat::Sig)), ".000");
    assert_eq!(format_cell(&num(0.0499, CellFormat::Sig)), ".050");
    assert_eq!(format_cell(&num(0.9994, CellFormat::Sig)), ".999");
    assert_eq!(format_cell(&num(0.9996, CellFormat::Sig)), "1.000");
}

#[test]
fn count_nan_and_footnotes() {
    assert_eq!(format_cell(&num(42.0, CellFormat::Count)), "42");
    assert_eq!(format_cell(&num(f64::NAN, CellFormat::Fixed(3))), ".");
    let c = Cell { value: CellValue::Num(0.912), format: CellFormat::Fixed(3), footnotes: vec!['a'] };
    assert_eq!(format_cell(&c), ".912a");
    let c = Cell { value: CellValue::Num(0.2), format: CellFormat::Fixed(3), footnotes: vec!['a', '*'] };
    assert_eq!(format_cell(&c), ".200a,*");
    let t = Cell { value: CellValue::Text("x".into()), format: CellFormat::Text, footnotes: vec![] };
    assert_eq!(format_cell(&t), "x");
}

fn table(foot: bool) -> Table {
    let h = |s: &str| HeaderNode { label: s.into(), footnotes: vec![], children: vec![] };
    Table {
        title_footnotes: vec![],
        title: "Uji <A>".into(),
        col_headers: vec![h("Statistik"), h("Sig.")],
        row_headers: vec![h("X"), h("Y")],
        cells: vec![
            vec![num(0.912, CellFormat::Fixed(3)), num(0.01, CellFormat::Sig)],
            vec![num(-0.358, CellFormat::Fixed(3)), num(0.5, CellFormat::Sig)],
        ],
        footnotes: if foot { vec![('a', "Catatan".into())] } else { vec![] },
    }
}

#[test]
fn html_table() {
    let html = render_html(&[OutputItem::Table(table(true))]);
    assert!(html.contains("<table"));
    assert!(html.contains("Uji &lt;A&gt;"));
    assert!(html.contains("<tfoot>"));
    assert!(html.contains(".912"));
    let html = render_html(&[OutputItem::Table(table(false))]);
    assert!(!html.contains("<tfoot>"));
}

#[test]
fn text_table() {
    let t = render_text(&[
        OutputItem::Title("Judul".into()),
        OutputItem::Table(table(true)),
        OutputItem::Chart(Chart::Scatter { title: "S".into(), subtitle: None, x_label: "x".into(), y_label: "y".into(), points: vec![] }),
    ]);
    assert!(t.contains("Judul"));
    assert!(t.contains("-.358"));
    assert!(t.contains("a. Catatan"));
    assert!(t.contains("[Chart: S]"));
}

#[test]
fn nested_headers() {
    let mut t = table(false);
    t.col_headers = vec![HeaderNode { label: "Grup".into(), footnotes: vec![], children: vec![
        HeaderNode { label: "A".into(), footnotes: vec![], children: vec![] },
        HeaderNode { label: "B".into(), footnotes: vec![], children: vec![] }] }];
    let html = render_html(&[OutputItem::Table(t.clone())]);
    assert!(html.contains("colspan=\"2\""));
    let txt = render_text(&[OutputItem::Table(t)]);
    assert!(txt.contains("Grup"));
}

#[test]
fn regressions_count_text_rounding() {
    assert_eq!(format_cell(&num(-0.4, CellFormat::Count)), "0");
    assert_eq!(format_cell(&num(f64::INFINITY, CellFormat::Text)), ".");
    assert_eq!(format_cell(&num(f64::NAN, CellFormat::Text)), ".");
    assert_eq!(format_cell(&num(f64::NEG_INFINITY, CellFormat::Text)), ".");
    assert_eq!(format_cell(&num(-2.5, CellFormat::Fixed(0))), "-3");
    assert_eq!(format_cell(&num(0.0005, CellFormat::Sig)), ".001");
}

#[test]
fn fixed_huge_values_keep_digits() {
    assert_eq!(format_cell(&num(1e20, CellFormat::Fixed(3))), "100000000000000000000.000");
    assert_eq!(format_cell(&num(-1e20, CellFormat::Fixed(3))), "-100000000000000000000.000");
    assert_eq!(format_cell(&num(123456789.125, CellFormat::Fixed(2))), "123456789.13");
}

#[test]
fn multiple_markers_are_comma_joined() {
    let c = Cell { value: CellValue::Num(0.2), format: CellFormat::Sig, footnotes: vec!['c', 'd'] };
    assert_eq!(format_cell(&c), ".200c,d");
}

fn marked_table() -> Table {
    let h = |s: &str| HeaderNode { label: s.into(), footnotes: vec![], children: vec![] };
    Table {
        title_footnotes: vec![],
        title: "T".into(),
        col_headers: vec![h("Sig.")],
        row_headers: vec![h("r"), h("s")],
        cells: vec![vec![
            Cell { value: CellValue::Text("x".into()), format: CellFormat::Text, footnotes: vec!['b'] },
        ], vec![
            Cell { value: CellValue::Num(0.2), format: CellFormat::Sig, footnotes: vec!['c', 'd'] },
        ]],
        footnotes: vec![],
    }
}

#[test]
fn html_markers_are_superscript() {
    let html = render_html(&[OutputItem::Table(marked_table())]);
    assert!(html.contains(">x<sup>b</sup></td>"), "{html}");
    assert!(html.contains(">.200<sup>c,d</sup></td>"), "{html}");
}

#[test]
fn text_markers_are_comma_joined_letters() {
    let txt = render_text(&[OutputItem::Table(marked_table())]);
    assert!(txt.contains(".200c,d"), "{txt}");
    assert!(txt.contains("xb"), "{txt}");
}
