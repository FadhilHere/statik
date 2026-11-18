use dataset::*;
use output_model::*;
use procedures::explore::{ExploreOptions, run_explore};
use procedures::npar_ks::{NparKsOptions, run_npar_ks};
use std::path::Path;

fn load(name: &str) -> Dataset {
    let p = format!("{}/../../fixtures/data/{name}", env!("CARGO_MANIFEST_DIR"));
    load_csv(Path::new(&p)).unwrap()
}

fn small_ds(cols: &[(&str, Vec<f64>)]) -> Dataset {
    let mut ds = Dataset::new();
    for (name, v) in cols {
        ds.add_variable(Variable::numeric(name), Column::Numeric(v.clone())).unwrap();
    }
    ds
}

fn tables(items: &[OutputItem]) -> Vec<&Table> {
    items.iter().filter_map(|i| if let OutputItem::Table(t) = i { Some(t) } else { None }).collect()
}

fn table<'a>(items: &'a [OutputItem], title: &str) -> &'a Table {
    tables(items).into_iter().find(|t| t.title == title).unwrap_or_else(|| panic!("no table {title}"))
}

fn warnings(items: &[OutputItem]) -> Vec<&String> {
    items.iter().filter_map(|i| if let OutputItem::Warning(w) = i { Some(w) } else { None }).collect()
}

fn row(t: &Table, r: usize) -> Vec<String> {
    t.cells[r].iter().map(format_cell).collect()
}

fn explore(ds: &Dataset, vars: &[&str]) -> Vec<OutputItem> {
    let opt = ExploreOptions {
        dependents: vars.iter().map(|n| ds.find(n).unwrap()).collect(),
        normality: true,
        qq_plot: true,
    };
    run_explore(ds, &opt)
}

#[test]
fn explore_skripsi60_x1() {
    let ds = load("skripsi60.csv");
    let items = explore(&ds, &["x1"]);
    assert_eq!(items[0], OutputItem::Title("Explore".into()));
    assert!(warnings(&items).is_empty());

    let cps = table(&items, "Case Processing Summary");
    assert_eq!(cps.row_headers[0].label, "x1");
    assert_eq!(row(cps, 0), ["60", "100.0%", "0", "0.0%", "60", "100.0%"]);

    let d = table(&items, "Descriptives");
    assert_eq!(d.cells.len(), 13);
    assert_eq!(d.col_headers.iter().map(|h| h.label.as_str()).collect::<Vec<_>>(), ["Statistic", "Std. Error"]);
    let x1 = &d.row_headers[0];
    assert_eq!(x1.label, "x1");
    assert_eq!(x1.children[1].label, "95% Confidence Interval for Mean");
    assert_eq!(x1.children[1].children[0].label, "Lower Bound");
    // Mean 48.612667 (x1 has 2 decimals → mean shown with 4).
    assert_eq!(row(d, 0)[0], "48.6127");
    assert!(matches!(d.cells[0][1].value, CellValue::Num(_)));
    let var = match d.cells[5][0].value {
        CellValue::Num(v) => v,
        _ => panic!(),
    };
    assert!((var - 125.37581988700565).abs() < 1e-9);
    assert_eq!(row(d, 8)[0], "79.70", "maximum");
    assert_eq!(row(d, 6)[0], "11.19713", "std. deviation");
    assert_eq!(d.cells[2][1].value, CellValue::Empty, "no SE for CI upper bound");
    assert!(matches!(d.cells[11][1].value, CellValue::Num(_)), "SE skewness");
    assert!(matches!(d.cells[12][1].value, CellValue::Num(_)), "SE kurtosis");

    let t = table(&items, "Tests of Normality");
    // K-S: D = .117, df 60, Sig .041 (fixture p = 0.0406); S-W: W = .974, Sig .218.
    assert_eq!(row(t, 0), [".117", "60", ".041", ".974", "60", ".218"]);
    assert_eq!(t.col_headers[0].label, "Kolmogorov-Smirnov");
    assert!(t.footnotes.iter().any(|(m, s)| *m == 'a' && s == "Lilliefors Significance Correction"));
    assert!(!t.footnotes.iter().any(|(m, _)| *m == '*'), "no lower bound note when not needed");

    let qq = items
        .iter()
        .find_map(|i| if let OutputItem::Chart(c @ Chart::QQPlot { .. }) = i { Some(c) } else { None })
        .unwrap();
    let Chart::QQPlot { title, observed, expected } = qq else { unreachable!() };
    assert_eq!(title, "Normal Q-Q Plot of x1");
    assert_eq!(observed.len(), 60);
    assert!(observed.windows(2).all(|w| w[0] <= w[1]));
    // Blom: first expected = Φ⁻¹((1 − 3/8) / 60.25).
    assert!((expected[0] - stat_core::normal_quantile(0.625 / 60.25)).abs() < 1e-12);
}

#[test]
fn explore_lower_bound_sig() {
    let ds = load("sleep.csv");
    let items = explore(&ds, &["extra"]);
    let t = table(&items, "Tests of Normality");
    assert_eq!(row(t, 0)[2], ".200*");
    assert!(t.footnotes.iter().any(|(m, s)| *m == '*' && s == "This is a lower bound of the true significance."));
}

#[test]
fn explore_listwise_missing_and_labels() {
    let mut ds = load("edge_cases.csv");
    let id = ds.find("withmiss").unwrap();
    ds.var_mut(id).label = Some("With missing".into());
    ds.var_mut(id).missing = MissingSpec::Discrete(vec![99.0]);
    let items = explore(&ds, &["withmiss", "y"]);
    let cps = table(&items, "Case Processing Summary");
    // Listwise over both dependents: 2 system-missing + 1 user-missing of 12.
    assert_eq!(row(cps, 0), ["9", "75.0%", "3", "25.0%", "12", "100.0%"]);
    assert_eq!(row(cps, 1), ["9", "75.0%", "3", "25.0%", "12", "100.0%"]);
    assert_eq!(cps.row_headers[0].label, "With missing");
    let t = table(&items, "Tests of Normality");
    assert_eq!(row(t, 0)[1], "9");
    let qq_title = |i: &OutputItem| match i {
        OutputItem::Chart(Chart::QQPlot { title, .. }) => Some(title.clone()),
        _ => None,
    };
    assert!(items.iter().filter_map(qq_title).any(|t| t == "Normal Q-Q Plot of With missing"));
}

#[test]
fn explore_edge_cases_never_panic() {
    // Constant variable: warning, no normality row for it.
    let ds = load("edge_cases.csv");
    let items = explore(&ds, &["const"]);
    assert!(!warnings(&items).is_empty());
    assert!(tables(&items).iter().all(|t| t.title != "Tests of Normality"));

    // n = 4: K-S not computable ("."), Shapiro-Wilk is.
    let ds = small_ds(&[("v", vec![1.0, 2.0, 4.0, 7.0])]);
    let items = explore(&ds, &["v"]);
    let t = table(&items, "Tests of Normality");
    let r = row(t, 0);
    assert_eq!((r[0].as_str(), r[2].as_str()), (".", "."));
    assert_ne!(r[3], ".");
    assert!(!warnings(&items).is_empty());

    // No valid cases at all.
    let ds = small_ds(&[("v", vec![f64::NAN; 3])]);
    let items = explore(&ds, &["v"]);
    assert!(!warnings(&items).is_empty());

    // Single case.
    let ds = small_ds(&[("v", vec![5.0])]);
    let _ = explore(&ds, &["v"]);
}

#[test]
fn npar_ks_table() {
    let ds = load("sleep.csv");
    let opt = NparKsOptions { vars: vec![ds.find("extra").unwrap()] };
    let items = run_npar_ks(&ds, &opt);
    assert!(warnings(&items).is_empty());
    let t = table(&items, "One-Sample Kolmogorov-Smirnov Test");
    assert_eq!(t.col_headers[0].label, "extra");
    let rows: Vec<String> = t.cells.iter().map(|r| format_cell(&r[0])).collect();
    // extra has 1 decimal: mean with 3, sd with 4 decimals.
    assert_eq!(rows[0], "20");
    assert_eq!(rows[1], "1.540");
    assert_eq!(rows[2], "2.0179");
    assert_eq!(rows[3], ".143");
    assert_eq!(rows[6], ".143");
    assert!(rows[7].starts_with(".200"), "{}", rows[7]);
    assert_eq!(t.cells[7][0].footnotes, vec!['c', 'd']);
    assert_eq!(rows[7], ".200cd");
    let notes: Vec<&str> = t.footnotes.iter().map(|(_, s)| s.as_str()).collect();
    assert_eq!(
        notes,
        [
            "Test distribution is Normal.",
            "Calculated from data.",
            "Lilliefors Corrected.",
            "This is a lower bound of the true significance."
        ]
    );
}

#[test]
fn npar_ks_x1() {
    let ds = load("skripsi60.csv");
    let opt = NparKsOptions { vars: vec![ds.find("x1").unwrap()] };
    let items = run_npar_ks(&ds, &opt);
    let t = table(&items, "One-Sample Kolmogorov-Smirnov Test");
    assert_eq!(format_cell(&t.cells[7][0]), ".041c");
    assert_eq!(t.footnotes.len(), 3);
}

#[test]
fn npar_ks_on_saved_regression_residuals() {
    use procedures::regression::{RegressionOptions, SaveFlags, run_regression};
    let mut ds = load("skripsi60.csv");
    let opt = RegressionOptions {
        dependent: ds.find("y").unwrap(),
        independents: ["x1", "x2", "x3"].iter().map(|n| ds.find(n).unwrap()).collect(),
        collinearity: false,
        durbin_watson: false,
        plot_hist: false,
        plot_pp: false,
        plot_zpred_sresid: false,
        save: SaveFlags { residual: true, ..SaveFlags::default() },
    };
    run_regression(&mut ds, &opt);
    let res = ds.find("RES_1").unwrap();
    let items = run_npar_ks(&ds, &NparKsOptions { vars: vec![res] });
    let t = table(&items, "One-Sample Kolmogorov-Smirnov Test");
    assert_eq!(t.col_headers[0].label, "Unstandardized Residual");
    let rows: Vec<String> = t.cells.iter().map(|r| format_cell(&r[0])).collect();
    // RES_1 has 5 decimals: mean with 7, sd with 8. statsmodels D = 0.068966, p > .2.
    assert_eq!(rows[0], "60");
    assert_eq!(rows[1], ".0000000");
    assert_eq!(rows[2].split('.').nth(1).unwrap().len(), 8, "{}", rows[2]);
    assert_eq!(rows[3], ".069");
    assert_eq!(rows[7], ".200cd");
}

#[test]
fn npar_ks_constant_warns() {
    let ds = load("edge_cases.csv");
    let opt = NparKsOptions { vars: vec![ds.find("const").unwrap()] };
    let items = run_npar_ks(&ds, &opt);
    assert!(!warnings(&items).is_empty());
    assert!(tables(&items).is_empty());

    // Mixed: one good variable keeps its column, the constant one is dropped with a warning.
    let opt = NparKsOptions { vars: vec![ds.find("y").unwrap(), ds.find("const").unwrap()] };
    let items = run_npar_ks(&ds, &opt);
    assert_eq!(warnings(&items).len(), 1);
    assert_eq!(table(&items, "One-Sample Kolmogorov-Smirnov Test").col_headers.len(), 1);

    // Constant whose computed sd is a tiny non-zero number (rounding in the mean).
    let ds = small_ds(&[("v", vec![0.1; 6])]);
    let items = run_npar_ks(&ds, &NparKsOptions { vars: vec![VarId(0)] });
    let w = warnings(&items);
    assert_eq!(w.len(), 1);
    assert!(w[0].contains("constant"), "{}", w[0]);
    assert!(tables(&items).is_empty());

    // Huge values: sd overflows to infinity; the message must not claim the variable is constant.
    let ds = small_ds(&[("v", vec![-1e308, 1e308, 0.0, 1.0, 2.0])]);
    let items = run_npar_ks(&ds, &NparKsOptions { vars: vec![VarId(0)] });
    let w = warnings(&items);
    assert_eq!(w.len(), 1);
    assert!(!w[0].contains("constant") && w[0].contains("not finite"), "{}", w[0]);

    // Too few cases.
    let ds = small_ds(&[("v", vec![1.0, 2.0, 4.0])]);
    let items = run_npar_ks(&ds, &NparKsOptions { vars: vec![VarId(0)] });
    assert!(!warnings(&items).is_empty());
}
