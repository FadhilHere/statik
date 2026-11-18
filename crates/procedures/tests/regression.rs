use dataset::*;
use output_model::*;
use procedures::regression::{RegressionOptions, SaveFlags, run_regression};
use std::path::Path;

fn load(name: &str) -> Dataset {
    let p = format!("{}/../../fixtures/data/{name}", env!("CARGO_MANIFEST_DIR"));
    load_csv(Path::new(&p)).unwrap()
}

fn refs() -> serde_json::Value {
    let p = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/ref/ols_skripsi60.json");
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn all_on(ds: &Dataset, dep: &str, ind: &[&str]) -> RegressionOptions {
    RegressionOptions {
        dependent: ds.find(dep).unwrap(),
        independents: ind.iter().map(|n| ds.find(n).unwrap()).collect(),
        collinearity: true,
        durbin_watson: true,
        plot_hist: true,
        plot_pp: true,
        plot_zpred_sresid: true,
        save: SaveFlags { predicted: true, residual: true, z_predicted: true, z_residual: true, studentized: true },
    }
}

fn tables(items: &[OutputItem]) -> Vec<&Table> {
    items.iter().filter_map(|i| if let OutputItem::Table(t) = i { Some(t) } else { None }).collect()
}

fn table<'a>(items: &'a [OutputItem], title: &str) -> &'a Table {
    tables(items).into_iter().find(|t| t.title == title).unwrap_or_else(|| panic!("no table {title}"))
}

#[test]
fn skripsi60_full_output() {
    let mut ds = load("skripsi60.csv");
    let opt = all_on(&ds, "y", &["x1", "x2", "x3"]);
    let items = run_regression(&mut ds, &opt);
    let body: Vec<&OutputItem> = items.iter().filter(|i| !matches!(i, OutputItem::Title(_))).collect();
    let titles: Vec<&str> = body
        .iter()
        .take(6)
        .map(|i| match i {
            OutputItem::Table(t) => t.title.as_str(),
            other => panic!("expected table, got {other:?}"),
        })
        .collect();
    assert_eq!(
        titles,
        ["Variables Entered/Removed", "Model Summary", "ANOVA", "Coefficients", "Collinearity Diagnostics", "Residuals Statistics"]
    );
    assert_eq!(body.len(), 9);
    let charts: Vec<&Chart> =
        body[6..].iter().map(|i| if let OutputItem::Chart(c) = i { c } else { panic!("expected chart") }).collect();
    assert!(matches!(charts[0], Chart::Histogram { normal_curve: true, values, .. } if values.len() == 60));
    assert_eq!(charts[0].title(), "Histogram");
    assert_eq!(charts[0].subtitle(), Some("Dependent Variable: y"));
    match charts[1] {
        Chart::PPPlot { title, observed_cum, expected_cum, .. } => {
            assert!(title.starts_with("Normal P-P Plot of Regression Standardized Residual"));
            assert_eq!(observed_cum.len(), 60);
            assert_eq!(expected_cum.len(), 60);
            assert!(observed_cum.windows(2).all(|w| w[0] < w[1]));
            assert!(expected_cum.windows(2).all(|w| w[0] <= w[1]));
        }
        c => panic!("expected P-P plot, got {c:?}"),
    }
    match charts[2] {
        Chart::Scatter { x_label, y_label, points, .. } => {
            assert_eq!(x_label, "Regression Standardized Predicted Value");
            assert_eq!(y_label, "Regression Studentized Residual");
            assert_eq!(points.len(), 60);
        }
        c => panic!("expected scatter, got {c:?}"),
    }

    let r = refs();
    let ms = table(&items, "Model Summary");
    let r2 = r["values"]["rsquared"].as_f64().unwrap();
    let expect = format_cell(&Cell { value: CellValue::Num(r2), format: CellFormat::Fixed(3), footnotes: vec![] });
    assert_eq!(format_cell(&ms.cells[0][1]), expect);
    assert_eq!(ms.cells[0].len(), 5); // R, R2, adj R2, SE, DW
    let dw = r["values"]["durbin_watson"].as_f64().unwrap();
    assert_eq!(
        format_cell(&ms.cells[0][4]),
        format_cell(&Cell { value: CellValue::Num(dw), format: CellFormat::Fixed(3), footnotes: vec![] })
    );
    assert!(ms.footnotes.iter().any(|(_, s)| s == "Predictors: (Constant), x3, x2, x1"));
    assert!(ms.footnotes.iter().any(|(_, s)| s == "Dependent Variable: y"));

    let anova = table(&items, "ANOVA");
    assert_eq!(anova.cells.len(), 3);
    assert_eq!(format_cell(&anova.cells[0][1]), "3");
    assert_eq!(format_cell(&anova.cells[1][1]), "56");
    assert_eq!(format_cell(&anova.cells[2][1]), "59");

    let coef = table(&items, "Coefficients");
    assert_eq!(coef.cells.len(), 4);
    assert_eq!(coef.cells[0].len(), 7); // B, SE, Beta, t, Sig, Tolerance, VIF
    assert_eq!(coef.cells[0][2].value, CellValue::Empty); // Beta of constant
    assert_eq!(coef.cells[0][6].value, CellValue::Empty); // VIF of constant

    let cd = table(&items, "Collinearity Diagnostics");
    assert_eq!(cd.cells.len(), 4);
    assert_eq!(cd.cells[0].len(), 2 + 4);

    let rs = table(&items, "Residuals Statistics");
    assert_eq!(rs.cells.len(), 4);
    assert_eq!(format_cell(&rs.cells[0][4]), "60");

    // saved variables
    for name in ["PRE_1", "RES_1", "ZPR_1", "ZRE_1", "SRE_1"] {
        assert!(ds.find(name).is_some(), "{name} missing");
    }
    assert_eq!(ds.var(ds.find("RES_1").unwrap()).label.as_deref(), Some("Unstandardized Residual"));
    assert_eq!(ds.var(ds.find("SRE_1").unwrap()).label.as_deref(), Some("Studentized Residual"));

    let _ = run_regression(&mut ds, &opt);
    assert!(ds.find("RES_2").is_some());
    assert!(ds.find("PRE_2").is_some());
}

#[test]
fn coefficient_b_close_to_reference() {
    let mut ds = load("skripsi60.csv");
    let opt = all_on(&ds, "y", &["x1", "x2", "x3"]);
    let items = run_regression(&mut ds, &opt);
    let coef = table(&items, "Coefficients");
    let r = refs();
    for j in 0..4 {
        let CellValue::Num(b) = coef.cells[j][0].value else { panic!() };
        let e = r["values"]["params"][j].as_f64().unwrap();
        assert!((b - e).abs() <= 1e-9 * e.abs().max(1.0));
    }
}

#[test]
fn user_missing_listwise() {
    let mut ds = load("edge_cases.csv");
    let wm = ds.find("withmiss").unwrap();
    ds.var_mut(wm).missing = MissingSpec::Discrete(vec![99.0]);
    let opt = all_on(&ds, "y", &["x1", "withmiss"]);
    let items = run_regression(&mut ds, &opt);
    let rs = table(&items, "Residuals Statistics");
    for row in &rs.cells {
        assert_eq!(format_cell(&row[4]), "9");
    }
    let res = ds.numeric(ds.find("RES_1").unwrap());
    for row in [2, 4, 7] {
        assert!(res[row].is_nan(), "row {row} should be missing");
    }
    assert_eq!(res.iter().filter(|v| !v.is_nan()).count(), 9);
}

#[test]
fn excluded_variables_table() {
    let mut ds = load("edge_cases.csv");
    let opt = all_on(&ds, "y", &["x1", "x1dbl"]);
    let items = run_regression(&mut ds, &opt);
    let titles: Vec<&str> = tables(&items).iter().map(|t| t.title.as_str()).collect();
    assert_eq!(
        titles,
        [
            "Variables Entered/Removed",
            "Model Summary",
            "ANOVA",
            "Coefficients",
            "Excluded Variables",
            "Collinearity Diagnostics",
            "Residuals Statistics"
        ]
    );
    let ex = table(&items, "Excluded Variables");
    assert_eq!(ex.cells.len(), 1);
    assert_eq!(table(&items, "Coefficients").cells.len(), 2);
}

#[test]
fn too_few_cases_warns() {
    let mut ds = Dataset::new();
    for (name, v) in [("y", [1.0, 2.0, 4.0]), ("a", [1.0, 2.0, 3.0]), ("b", [3.0, 1.0, 2.0]), ("c", [5.0, 5.0, 1.0])] {
        ds.add_variable(Variable::numeric(name), Column::Numeric(v.to_vec())).unwrap();
    }
    let opt = all_on(&ds, "y", &["a", "b", "c"]);
    let items = run_regression(&mut ds, &opt);
    assert_eq!(items.len(), 1);
    assert!(matches!(items[0], OutputItem::Warning(_)));
    assert_eq!(ds.n_vars(), 4, "nothing saved on failure");
}

#[test]
fn minimal_options_and_labels() {
    let mut ds = load("skripsi60.csv");
    let y = ds.find("y").unwrap();
    ds.var_mut(y).label = Some("Kepuasan".into());
    let opt = RegressionOptions {
        dependent: y,
        independents: vec![ds.find("x1").unwrap()],
        collinearity: false,
        durbin_watson: false,
        plot_hist: false,
        plot_pp: false,
        plot_zpred_sresid: false,
        save: SaveFlags::default(),
    };
    let n_vars = ds.n_vars();
    let items = run_regression(&mut ds, &opt);
    let titles: Vec<&str> = tables(&items).iter().map(|t| t.title.as_str()).collect();
    assert_eq!(titles, ["Variables Entered/Removed", "Model Summary", "ANOVA", "Coefficients"]);
    assert!(!items.iter().any(|i| matches!(i, OutputItem::Chart(_))));
    assert_eq!(table(&items, "Model Summary").cells[0].len(), 4);
    assert_eq!(table(&items, "Coefficients").cells[0].len(), 5);
    assert!(table(&items, "ANOVA").footnotes.iter().any(|(_, s)| s == "Dependent Variable: Kepuasan"));
    assert_eq!(ds.n_vars(), n_vars);
}

fn small_ds(cols: &[(&str, Vec<f64>)]) -> Dataset {
    let mut ds = Dataset::new();
    for (name, v) in cols {
        ds.add_variable(Variable::numeric(name), Column::Numeric(v.clone())).unwrap();
    }
    ds
}

#[test]
fn zero_variance_dependent_warns() {
    let mut ds = small_ds(&[("y", vec![2.7; 6]), ("x", vec![1.0, 2.0, 3.0, 5.0, 4.0, 6.0])]);
    let opt = all_on(&ds, "y", &["x"]);
    let items = run_regression(&mut ds, &opt);
    assert_eq!(items.len(), 1);
    match &items[0] {
        OutputItem::Warning(w) => assert!(w.contains("zero variance"), "{w}"),
        other => panic!("expected warning, got {other:?}"),
    }
    assert_eq!(ds.n_vars(), 2);
}

#[test]
fn perfect_fit_no_nonfinite_cells_or_chart_values() {
    let x = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
    let y: Vec<f64> = x.iter().map(|v| 1.5 + 0.3 * v).collect();
    let mut ds = small_ds(&[("y", y), ("x", x)]);
    let opt = all_on(&ds, "y", &["x"]);
    let items = run_regression(&mut ds, &opt);
    assert!(items.iter().any(|i| matches!(i, OutputItem::Warning(_))));
    assert!(tables(&items).len() >= 4);
    for t in tables(&items) {
        for row in &t.cells {
            for c in row {
                if let CellValue::Num(v) = c.value {
                    assert!(v.is_finite(), "non-finite cell in {}", t.title);
                }
            }
        }
    }
    for i in &items {
        if let OutputItem::Chart(c) = i {
            let ok = match c {
                Chart::Histogram { values, .. } => !values.is_empty() && values.iter().all(|v| v.is_finite()),
                Chart::PPPlot { observed_cum, expected_cum, .. } => {
                    !observed_cum.is_empty() && observed_cum.iter().chain(expected_cum).all(|v| v.is_finite())
                }
                Chart::Scatter { points, .. } => !points.is_empty() && points.iter().all(|(a, b)| a.is_finite() && b.is_finite()),
                Chart::QQPlot { .. } => true,
            };
            assert!(ok, "chart with non-finite or no data: {c:?}");
        }
    }
}

#[test]
fn zero_slope_has_defined_r_and_no_zpred_noise() {
    let mut ds = small_ds(&[("y", vec![0.1, 0.2, 0.3, 0.2, 0.1]), ("x", vec![0.1, 0.2, 0.3, 0.4, 0.5])]);
    let opt = all_on(&ds, "y", &["x"]);
    let items = run_regression(&mut ds, &opt);
    let ms = table(&items, "Model Summary");
    assert_eq!(format_cell(&ms.cells[0][0]), ".000a");
    let rs = table(&items, "Residuals Statistics");
    assert!(rs.cells[2][..4].iter().all(|c| format_cell(c) == "."), "{:?}", rs.cells[2]);
    let zpr = ds.find("ZPR_1").expect("ZPR_1 saved");
    assert!(ds.numeric(zpr).iter().all(|v| v.is_nan()));
    assert!(!items.iter().any(|i| matches!(i, OutputItem::Chart(Chart::Scatter { .. }))));
}
