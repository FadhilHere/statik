use dataset::*;
use output_model::*;
use procedures::ttest_ind::{IndependentOptions, run_independent};
use procedures::ttest_one::{OneSampleOptions, run_one_sample};
use procedures::ttest_paired::{PairedOptions, run_paired};
use std::path::Path;

fn load(name: &str) -> Dataset {
    let p = format!("{}/../../fixtures/data/{name}", env!("CARGO_MANIFEST_DIR"));
    load_csv(Path::new(&p)).unwrap()
}

fn tables(items: &[OutputItem]) -> Vec<&Table> {
    items.iter().filter_map(|i| if let OutputItem::Table(t) = i { Some(t) } else { None }).collect()
}

fn warnings(items: &[OutputItem]) -> Vec<&str> {
    items.iter().filter_map(|i| if let OutputItem::Warning(w) = i { Some(w.as_str()) } else { None }).collect()
}

fn row_labels(t: &Table) -> Vec<String> {
    fn walk(n: &HeaderNode, out: &mut Vec<String>) {
        if n.children.is_empty() {
            out.push(n.label.clone());
        }
        for c in &n.children {
            walk(c, out);
        }
    }
    let mut v = vec![];
    t.row_headers.iter().for_each(|n| walk(n, &mut v));
    v
}

fn num(c: &Cell) -> f64 {
    match c.value {
        CellValue::Num(v) => v,
        ref o => panic!("not numeric: {o:?}"),
    }
}

#[test]
fn independent_sleep_tables() {
    let ds = load("sleep.csv");
    let o = IndependentOptions {
        test_vars: vec![ds.find("extra").unwrap()],
        grouping: ds.find("group").unwrap(),
        group1: 1.0,
        group2: 2.0,
        conf: 0.95,
    };
    let items = run_independent(&ds, &o);
    let ts = tables(&items);
    assert_eq!(ts[0].title, "Group Statistics");
    assert_eq!(ts[1].title, "Independent Samples Test");
    assert_eq!(row_labels(ts[1]), ["Equal variances assumed", "Equal variances not assumed"]);
    assert_eq!(row_labels(ts[0]).len(), 2);
    // N column of group statistics
    assert_eq!(num(&ts[0].cells[0][0]), 10.0);
    // t cell of the first row (columns: F, Sig, t, df, p1, p2, md, se, lo, hi)
    let row = &ts[1].cells[0];
    assert!((num(&row[2]) + 1.8608).abs() < 1e-4);
    assert!((num(&row[5]) - 0.07919).abs() < 1e-5);
    assert!((num(&row[4]) - 0.07919 / 2.0).abs() < 1e-5);
    let w = &ts[1].cells[1];
    assert!(matches!(w[0].value, CellValue::Empty));
    assert!((num(&w[3]) - 17.776).abs() < 1e-3);
}

#[test]
fn independent_missing_group_warns() {
    let ds = load("sleep.csv");
    let o = IndependentOptions {
        test_vars: vec![ds.find("extra").unwrap()],
        grouping: ds.find("group").unwrap(),
        group1: 1.0,
        group2: 9.0,
        conf: 0.95,
    };
    let items = run_independent(&ds, &o);
    assert!(!warnings(&items).is_empty());
    assert!(tables(&items).iter().all(|t| t.title != "Independent Samples Test"));
}

#[test]
fn one_sample_constant_warns() {
    let ds = load("edge_cases.csv");
    let o = OneSampleOptions { vars: vec![ds.find("const").unwrap()], test_value: 0.0, conf: 0.95 };
    let items = run_one_sample(&ds, &o);
    assert!(warnings(&items).contains(&"t cannot be computed because the standard deviation is 0."));
    let ts = tables(&items);
    assert_eq!(ts.len(), 1);
    assert_eq!(ts[0].title, "One-Sample Statistics");
}

#[test]
fn one_sample_tables() {
    let ds = load("skripsi60.csv");
    let o = OneSampleOptions { vars: vec![ds.find("x1").unwrap()], test_value: 50.0, conf: 0.95 };
    let items = run_one_sample(&ds, &o);
    let ts = tables(&items);
    assert_eq!(ts[0].title, "One-Sample Statistics");
    assert_eq!(ts[1].title, "One-Sample Test");
    assert_eq!(ts[1].col_headers[0].label, "Test Value = 50");
    assert_eq!(ts[1].cells[0].len(), 7);
    assert_eq!(num(&ts[0].cells[0][0]), 60.0);
}

#[test]
fn paired_sleep_tables() {
    // Two columns from skripsi60 as a pair.
    let ds = load("skripsi60.csv");
    let o = PairedOptions { pairs: vec![(ds.find("x1").unwrap(), ds.find("x2").unwrap())], conf: 0.95 };
    let items = run_paired(&ds, &o);
    let ts = tables(&items);
    let titles: Vec<&str> = ts.iter().map(|t| t.title.as_str()).collect();
    assert_eq!(titles, ["Paired Samples Statistics", "Paired Samples Correlations", "Paired Samples Test"]);
    assert_eq!(row_labels(ts[2]), ["Pair 1  x1 - x2"]);
    assert_eq!(row_labels(ts[1]), ["Pair 1  x1 & x2"]);
    assert_eq!(row_labels(ts[0]), ["x1", "x2"]);
    assert_eq!(ts[2].cells[0].len(), 9);
}

#[test]
fn paired_constant_difference_warns() {
    let ds = load("edge_cases.csv");
    let o = PairedOptions { pairs: vec![(ds.find("x1dbl").unwrap(), ds.find("x1").unwrap())], conf: 0.95 };
    let items = run_paired(&ds, &o);
    assert!(warnings(&items).is_empty()); // x1dbl - x1 = x1, not constant
    let o = PairedOptions { pairs: vec![(ds.find("const").unwrap(), ds.find("const").unwrap())], conf: 0.95 };
    let items = run_paired(&ds, &o);
    assert!(!warnings(&items).is_empty());
}

fn const_ds() -> Dataset {
    let mut ds = Dataset::new();
    ds.add_variable(Variable::numeric("c1"), Column::Numeric(vec![0.1; 3])).unwrap();
    ds.add_variable(Variable::numeric("c3"), Column::Numeric(vec![0.3; 3])).unwrap();
    ds.add_variable(Variable::numeric("g"), Column::Numeric(vec![1.0, 1.0, 2.0])).unwrap();
    ds
}

#[test]
fn inexact_constants_warn() {
    let ds = const_ds();
    let (c1, c3) = (ds.find("c1").unwrap(), ds.find("c3").unwrap());
    let items = run_one_sample(&ds, &OneSampleOptions { vars: vec![c1], test_value: 0.0, conf: 0.95 });
    assert!(warnings(&items).contains(&"t cannot be computed because the standard deviation is 0."));
    assert_eq!(tables(&items).len(), 1);
    let ds2 = {
        let mut d = Dataset::new();
        d.add_variable(Variable::numeric("y"), Column::Numeric(vec![0.1; 4])).unwrap();
        d.add_variable(Variable::numeric("g"), Column::Numeric(vec![1.0, 1.0, 2.0, 2.0])).unwrap();
        d
    };
    let items = run_independent(
        &ds2,
        &IndependentOptions {
            test_vars: vec![ds2.find("y").unwrap()],
            grouping: ds2.find("g").unwrap(),
            group1: 1.0,
            group2: 2.0,
            conf: 0.95,
        },
    );
    assert!(!warnings(&items).is_empty());
    assert!(tables(&items).iter().all(|t| t.title != "Independent Samples Test"));
    // Paired: constant columns -> statistics and correlation tables stay, the test becomes a warning.
    let items = run_paired(&ds, &PairedOptions { pairs: vec![(c3, c1)], conf: 0.95 });
    let titles: Vec<&str> = tables(&items).iter().map(|t| t.title.as_str()).collect();
    assert_eq!(titles, ["Paired Samples Statistics", "Paired Samples Correlations"]);
    assert!(!warnings(&items).is_empty());
    let corr = tables(&items)[1];
    assert!(matches!(&corr.cells[0][1].value, CellValue::Text(s) if s == "."));
}

#[test]
fn paired_constant_difference_keeps_correlation() {
    let mut ds = Dataset::new();
    ds.add_variable(Variable::numeric("a"), Column::Numeric(vec![1.0, 2.0, 4.0, 7.0])).unwrap();
    ds.add_variable(Variable::numeric("b"), Column::Numeric(vec![0.0, 1.0, 3.0, 6.0])).unwrap();
    let (a, b) = (ds.find("a").unwrap(), ds.find("b").unwrap());
    let items = run_paired(&ds, &PairedOptions { pairs: vec![(a, b)], conf: 0.95 });
    let titles: Vec<&str> = tables(&items).iter().map(|t| t.title.as_str()).collect();
    assert_eq!(titles, ["Paired Samples Statistics", "Paired Samples Correlations"]);
    assert!((num(&tables(&items)[1].cells[0][1]) - 1.0).abs() < 1e-9);
    assert!(!warnings(&items).is_empty());
}
