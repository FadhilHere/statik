use output_model::*;

fn hn(s: &str, f: &[char]) -> HeaderNode {
    HeaderNode { label: s.into(), footnotes: f.to_vec(), children: vec![] }
}

fn table() -> Table {
    Table {
        title: "Coefficients".into(),
        title_footnotes: vec!['a'],
        col_headers: vec![HeaderNode {
            label: "Grup".into(),
            footnotes: vec!['b', 'c'],
            children: vec![hn("A", &['a']), hn("B", &[])],
        }],
        row_headers: vec![hn("Baris", &['d'])],
        cells: vec![vec![
            Cell { value: CellValue::Num(f64::NAN), format: CellFormat::Fixed(3), footnotes: vec![] },
            Cell { value: CellValue::Num(1.5), format: CellFormat::Sig, footnotes: vec!['x'] },
        ]],
        footnotes: vec![('a', "Catatan".into())],
    }
}

#[test]
fn html_title_and_header_markers_are_sup() {
    let h = render_html(&[OutputItem::Table(table())]);
    assert!(h.contains("Coefficients<sup>a</sup>"), "{h}");
    assert!(h.contains("Grup<sup>b,c</sup>"), "{h}");
    assert!(h.contains("A<sup>a</sup>"), "{h}");
    assert!(h.contains("Baris<sup>d</sup>"), "{h}");
}

#[test]
fn text_title_and_header_markers_are_comma_separated() {
    let t = render_text(&[OutputItem::Table(table())]);
    assert!(t.starts_with("Coefficientsa
"), "{t}");
    assert!(t.contains("Barisd"), "{t}");
}

#[test]
fn chart_subtitle_accessor() {
    let c = Chart::Histogram { title: "H".into(), subtitle: Some("Dependent Variable: y".into()), x_label: String::new(), values: vec![], normal_curve: false };
    assert_eq!(c.subtitle(), Some("Dependent Variable: y"));
    assert_eq!(c.title(), "H");
}

#[test]
fn serde_round_trip_with_nan() {
    let items = vec![
        OutputItem::Title("T".into()),
        OutputItem::Table(table()),
        OutputItem::Chart(Chart::Scatter {
            title: "S".into(),
            subtitle: None,
            x_label: "x".into(),
            y_label: "y".into(),
            points: vec![(1.0, 2.0)],
        }),
        OutputItem::Warning("w".into()),
    ];
    let json = serde_json::to_string(&items).unwrap();
    let back: Vec<OutputItem> = serde_json::from_str(&json).unwrap();
    // NaN != NaN, so compare the NaN cell separately and the rest structurally.
    let (OutputItem::Table(a), OutputItem::Table(b)) = (&items[1], &back[1]) else { panic!() };
    assert!(matches!(b.cells[0][0].value, CellValue::Num(v) if v.is_nan()));
    assert_eq!(a.cells[0][1], b.cells[0][1]);
    assert_eq!(a.title_footnotes, b.title_footnotes);
    assert_eq!(a.col_headers, b.col_headers);
    assert_eq!(items[0], back[0]);
    assert_eq!(items[2], back[2]);
    assert_eq!(items[3], back[3]);
}

#[test]
fn serde_infinities() {
    for v in [f64::INFINITY, f64::NEG_INFINITY, 0.1, -3.25e300] {
        let c = Cell { value: CellValue::Num(v), format: CellFormat::Count, footnotes: vec![] };
        let back: Cell = serde_json::from_str(&serde_json::to_string(&c).unwrap()).unwrap();
        assert_eq!(c, back);
    }
}

#[test]
fn chart_data_with_nan_and_inf_round_trips() {
    let charts = vec![
        Chart::Histogram { title: "H".into(), subtitle: None, x_label: "x".into(), values: vec![1.0, f64::NAN, f64::INFINITY], normal_curve: true },
        Chart::PPPlot { title: "P".into(), subtitle: None, observed_cum: vec![f64::NAN], expected_cum: vec![0.5] },
        Chart::QQPlot { title: "Q".into(), subtitle: None, observed: vec![f64::NEG_INFINITY], expected: vec![f64::NAN] },
        Chart::Scatter { title: "S".into(), subtitle: None, x_label: "x".into(), y_label: "y".into(), points: vec![(f64::NAN, 2.5)] },
    ];
    let json = serde_json::to_string(&charts).unwrap();
    assert!(json.contains("\"NaN\"") && json.contains("\"inf\"") && json.contains("\"-inf\""), "{json}");
    let back: Vec<Chart> = serde_json::from_str(&json).unwrap();
    assert_eq!(format!("{back:?}"), format!("{charts:?}"));
    // Older files without x_label still load.
    let old = r#"{"Histogram":{"title":"H","subtitle":null,"values":[1.0,2.0],"normal_curve":false}}"#;
    let c: Chart = serde_json::from_str(old).unwrap();
    assert!(matches!(c, Chart::Histogram { ref x_label, ref values, .. } if x_label.is_empty() && values.len() == 2));
}
