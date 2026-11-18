use dataset::*;
use output_model::*;
use procedures::descriptives::{DescriptivesOptions, run_descriptives};
use procedures::frequencies::{FrequenciesOptions, run_frequencies};
use procedures::npar_runs::{RunsOptions, run_runs};
use procedures::oneway::{OneWayOptions, run_oneway};
use std::path::Path;

fn load(name: &str) -> Dataset {
    let p = format!("{}/../../fixtures/data/{name}", env!("CARGO_MANIFEST_DIR"));
    load_csv(Path::new(&p)).unwrap()
}

fn refs(case: &str) -> serde_json::Value {
    let p = format!("{}/../../fixtures/ref/{case}.json", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
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

fn flat(h: &[HeaderNode]) -> Vec<String> {
    fn go(n: &HeaderNode, o: &mut Vec<String>) {
        if n.children.is_empty() {
            o.push(n.label.clone());
        }
        n.children.iter().for_each(|c| go(c, o));
    }
    let mut o = vec![];
    h.iter().for_each(|n| go(n, &mut o));
    o
}

/// Cell by leaf row label and leaf column label. Labels that repeat ("Total") resolve to the first.
fn val(t: &Table, row: &str, col: &str) -> Cell {
    let r = flat(&t.row_headers).iter().position(|l| l == row).unwrap_or_else(|| panic!("no row {row}"));
    let c = flat(&t.col_headers).iter().position(|l| l == col).unwrap_or_else(|| panic!("no col {col}"));
    t.cells[r][c].clone()
}

fn n(c: &Cell) -> f64 {
    match c.value {
        CellValue::Num(v) => v,
        ref o => panic!("not numeric: {o:?}"),
    }
}

fn close(a: f64, b: f64, tol: f64) {
    assert!((a - b).abs() <= tol, "{a} vs {b}");
}

fn ow(ds: &Dataset) -> OneWayOptions {
    OneWayOptions { dependent: ds.find("y").unwrap(), factor: ds.find("grp").unwrap(), descriptives: true, homogeneity: true }
}

#[test]
fn oneway_tables() {
    let ds = load("skripsi60.csv");
    let items = run_oneway(&ds, &ow(&ds));
    let r = refs("anova");
    let v = &r["values"];
    let a = table(&items, "ANOVA");
    assert_eq!(flat(&a.row_headers), ["Between Groups", "Within Groups", "Total"]);
    close(n(&val(a, "Between Groups", "F")), v["f"].as_f64().unwrap(), 1e-9);
    close(n(&val(a, "Between Groups", "Sig.")), v["p"].as_f64().unwrap(), 1e-10);
    assert_eq!(n(&val(a, "Within Groups", "df")), 58.0);
    assert_eq!(n(&val(a, "Total", "df")), 59.0);
    close(n(&val(a, "Total", "Sum of Squares")), v["ssb"].as_f64().unwrap() + v["ssw"].as_f64().unwrap(), 1e-8);
    assert_eq!(val(a, "Within Groups", "F").value, CellValue::Empty);
    let h = table(&items, "Tests of Homogeneity of Variances");
    close(n(&val(h, "Based on Mean", "Levene Statistic")), v["levene_f"].as_f64().unwrap(), 1e-9);
    close(n(&val(h, "Based on Mean", "Sig.")), v["levene_p"].as_f64().unwrap(), 1e-9);
    assert_eq!((n(&val(h, "Based on Mean", "df1")), n(&val(h, "Based on Mean", "df2"))), (1.0, 58.0));
    let d = table(&items, "Descriptives");
    assert_eq!(flat(&d.row_headers), ["1", "2", "Total"]);
    assert_eq!(n(&val(d, "1", "N")), 30.0);
    assert_eq!(n(&val(d, "Total", "N")), 60.0);
    assert_eq!(val(d, "1", "Mean").format, CellFormat::Fixed(4));
    assert_eq!(val(d, "1", "Std. Deviation").format, CellFormat::Fixed(5));
    let g1: Vec<f64> = v["g1"].as_array().unwrap().iter().map(|e| e.as_f64().unwrap()).collect();
    close(n(&val(d, "1", "Mean")), g1.iter().sum::<f64>() / 30.0, 1e-9);
    assert!(n(&val(d, "1", "Lower Bound")) < n(&val(d, "1", "Mean")));
}

#[test]
fn oneway_options_off_and_labels() {
    let mut ds = load("skripsi60.csv");
    let g = ds.find("grp").unwrap();
    ds.var_mut(g).value_labels.insert(OrderedValue(1.0), "Kontrol".into());
    let mut o = ow(&ds);
    let items = run_oneway(&ds, &o);
    assert_eq!(flat(&table(&items, "Descriptives").row_headers), ["Kontrol", "2", "Total"]);
    o.descriptives = false;
    o.homogeneity = false;
    let items = run_oneway(&ds, &o);
    assert_eq!(tables(&items).len(), 1);
}

#[test]
fn oneway_degenerate() {
    let ds = load("edge_cases.csv");
    // single group: const has one distinct value
    let o = OneWayOptions { dependent: ds.find("y").unwrap(), factor: ds.find("const").unwrap(), descriptives: true, homogeneity: true };
    let items = run_oneway(&ds, &o);
    assert!(tables(&items).is_empty());
    assert_eq!(warnings(&items).len(), 1);
    // all groups constant: F undefined, shown as "."
    let mut ds = Dataset::new();
    ds.add_variable(Variable::numeric("v"), Column::Numeric(vec![2.0, 2.0, 5.0, 5.0])).unwrap();
    ds.add_variable(Variable::numeric("g"), Column::Numeric(vec![1.0, 1.0, 2.0, 2.0])).unwrap();
    let o = OneWayOptions { dependent: ds.find("v").unwrap(), factor: ds.find("g").unwrap(), descriptives: false, homogeneity: true };
    let items = run_oneway(&ds, &o);
    let a = table(&items, "ANOVA");
    assert_eq!(val(a, "Between Groups", "F").value, CellValue::Text(".".into()));
    assert!(!warnings(&items).is_empty());
}

fn resid_ds() -> Dataset {
    let r = refs("runs");
    let res: Vec<f64> = r["values"]["resid"].as_array().unwrap().iter().map(|e| e.as_f64().unwrap()).collect();
    let mut ds = Dataset::new();
    ds.add_variable(Variable::numeric("res"), Column::Numeric(res)).unwrap();
    ds
}

#[test]
fn runs_matches_reference() {
    let ds = resid_ds();
    let items = run_runs(&ds, &RunsOptions { vars: vec![ds.find("res").unwrap()] });
    let r = refs("runs");
    let v = &r["values"];
    let t = table(&items, "Runs Test");
    assert_eq!(
        flat(&t.row_headers),
        ["Test Value", "Cases < Test Value", "Cases >= Test Value", "Total Cases", "Number of Runs", "Z", "Asymp. Sig. (2-tailed)"]
    );
    close(n(&val(t, "Test Value", "res")), v["median"].as_f64().unwrap(), 1e-9);
    assert_eq!(n(&val(t, "Total Cases", "res")), 60.0);
    assert_eq!(n(&val(t, "Number of Runs", "res")), v["runs"].as_f64().unwrap());
    close(n(&val(t, "Z", "res")), v["z"].as_f64().unwrap(), 1e-9);
    close(n(&val(t, "Asymp. Sig. (2-tailed)", "res")), v["p"].as_f64().unwrap(), 1e-9);
    assert_eq!(t.footnotes, vec![('a', "Median".to_string())]);
}

#[test]
fn runs_constant_warns() {
    let ds = load("edge_cases.csv");
    let items = run_runs(&ds, &RunsOptions { vars: vec![ds.find("const").unwrap()] });
    assert!(tables(&items).is_empty());
    assert_eq!(warnings(&items).len(), 1);
}

#[test]
fn descriptives_table_and_save_z() {
    let mut ds = load("skripsi60.csv");
    let x1 = ds.find("x1").unwrap();
    let x2 = ds.find("x2").unwrap();
    let o = DescriptivesOptions { vars: vec![x1, x2], skew_kurt: true, save_z: true };
    let items = run_descriptives(&mut ds, &o);
    let t = table(&items, "Descriptive Statistics");
    assert_eq!(flat(&t.row_headers), ["x1", "x2", "Valid N (listwise)"]);
    assert_eq!(n(&val(t, "x1", "N")), 60.0);
    assert_eq!(val(t, "x1", "Mean").format, CellFormat::Fixed(4));
    assert_eq!(val(t, "x1", "Minimum").format, CellFormat::Fixed(2));
    assert_eq!(val(t, "Valid N (listwise)", "Mean").value, CellValue::Empty);
    assert_eq!(n(&val(t, "Valid N (listwise)", "N")), 60.0);
    let refd = refs("describe_skripsi60");
    close(n(&val(t, "x1", "Statistic")), refd["values"]["skew"].as_f64().unwrap(), 1e-9);
    let z = ds.find("Zx1").expect("Zx1 created");
    assert_eq!(ds.var(z).decimals, 5);
    assert_eq!(ds.var(z).label.as_deref(), Some("Zscore(x1)"));
    let zv = ds.numeric(z);
    close(zv.iter().sum::<f64>() / 60.0, 0.0, 1e-12);
    let sd = (zv.iter().map(|v| v * v).sum::<f64>() / 59.0).sqrt();
    close(sd, 1.0, 1e-12);
    assert!(ds.find("Zx2").is_some());
}

#[test]
fn descriptives_z_name_fallback_and_constant() {
    let mut ds = load("edge_cases.csv");
    let x1 = ds.find("x1").unwrap();
    ds.add_variable(Variable::numeric("Zx1"), Column::Numeric(vec![0.0; 12])).unwrap();
    let c = ds.find("const").unwrap();
    let items = run_descriptives(&mut ds, &DescriptivesOptions { vars: vec![x1, c], skew_kurt: false, save_z: true });
    assert!(ds.find("ZSC001").is_some());
    let zc = ds.find("Zconst").unwrap();
    assert!(ds.numeric(zc).iter().all(|v| v.is_nan()));
    assert_eq!(warnings(&items).len(), 1);
    let t = table(&items, "Descriptive Statistics");
    assert_eq!(n(&val(t, "const", "Std. Deviation")), 0.0);
}

#[test]
fn descriptives_missing_listwise() {
    let mut ds = load("edge_cases.csv");
    let w = ds.find("withmiss").unwrap();
    ds.var_mut(w).missing = MissingSpec::Discrete(vec![99.0]);
    let x1 = ds.find("x1").unwrap();
    let items = run_descriptives(&mut ds, &DescriptivesOptions { vars: vec![x1, w], skew_kurt: false, save_z: true });
    let t = table(&items, "Descriptive Statistics");
    assert_eq!(n(&val(t, "x1", "N")), 12.0);
    assert_eq!(n(&val(t, "withmiss", "N")), 9.0);
    assert_eq!(n(&val(t, "Valid N (listwise)", "N")), 9.0);
    let z = ds.numeric(ds.find("Zwithmiss").unwrap());
    assert!(z[2].is_nan() && z[4].is_nan() && z[7].is_nan() && z[0].is_finite());
}

#[test]
fn frequencies_missing_rows() {
    let mut ds = load("edge_cases.csv");
    let w = ds.find("withmiss").unwrap();
    ds.var_mut(w).missing = MissingSpec::Discrete(vec![99.0]);
    ds.var_mut(w).decimals = 1;
    let items = run_frequencies(&ds, &FrequenciesOptions { vars: vec![w], histogram: true });
    let s = table(&items, "Statistics");
    assert_eq!(n(&val(s, "Valid", "withmiss")), 9.0);
    assert_eq!(n(&val(s, "Missing", "withmiss")), 3.0);
    let t = table(&items, "withmiss");
    let rows = flat(&t.row_headers);
    assert_eq!(rows.len(), 9 + 1 + 2 + 1 + 1);
    assert!(rows.contains(&"10.5".to_string()) && rows.contains(&"System".to_string()));
    assert_eq!(rows.iter().filter(|r| *r == "Total").count(), 3);
    assert_eq!(n(&val(t, "System", "Frequency")), 2.0);
    assert_eq!(n(&val(t, "99.0", "Frequency")), 1.0);
    assert_eq!(val(t, "System", "Valid Percent").value, CellValue::Empty);
    assert_eq!(val(t, "10.5", "Percent").format, CellFormat::Fixed(1));
    close(n(&val(t, "10.5", "Percent")), 100.0 / 12.0, 1e-9);
    close(n(&val(t, "10.5", "Valid Percent")), 100.0 / 9.0, 1e-9);
    let last = t.cells.last().unwrap();
    close(n(&last[1]), 100.0, 1e-9);
    close(n(&val(t, "15.1", "Cumulative Percent")), 100.0, 1e-9);
    assert!(items
        .iter()
        .any(|i| matches!(i, OutputItem::Chart(Chart::Histogram { values, normal_curve: false, .. }) if values.len() == 9)));
}

#[test]
fn frequencies_labels_no_missing() {
    let mut ds = load("skripsi60.csv");
    let g = ds.find("grp").unwrap();
    ds.var_mut(g).value_labels.insert(OrderedValue(1.0), "Kontrol".into());
    let items = run_frequencies(&ds, &FrequenciesOptions { vars: vec![g], histogram: false });
    let t = table(&items, "grp");
    assert_eq!(flat(&t.row_headers), ["Kontrol", "2", "Total"]);
    assert_eq!(n(&val(t, "Kontrol", "Frequency")), 30.0);
    assert!(!items.iter().any(|i| matches!(i, OutputItem::Chart(_))));
    let s = table(&items, "Statistics");
    assert_eq!(n(&val(s, "Missing", "grp")), 0.0);
}
