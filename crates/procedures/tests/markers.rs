//! Footnote markers sit where SPSS puts them (titles, headers), and chart titles are one line.
use dataset::*;
use output_model::*;
use procedures::explore::{ExploreOptions, run_explore};
use procedures::frequencies::{FrequenciesOptions, run_frequencies};
use procedures::npar_ks::{NparKsOptions, run_npar_ks};
use procedures::npar_runs::{RunsOptions, run_runs};
use procedures::regression::{RegressionOptions, SaveFlags, run_regression};
use std::path::Path;

fn load(name: &str) -> Dataset {
    let p = format!("{}/../../fixtures/data/{name}", env!("CARGO_MANIFEST_DIR"));
    load_csv(Path::new(&p)).unwrap()
}

fn table<'a>(items: &'a [OutputItem], title: &str) -> &'a Table {
    items
        .iter()
        .find_map(|i| match i {
            OutputItem::Table(t) if t.title == title => Some(t),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no table {title}"))
}

fn find<'a>(nodes: &'a [HeaderNode], label: &str) -> &'a HeaderNode {
    for n in nodes {
        if n.label == label {
            return n;
        }
        if let Some(f) = n.children.iter().find(|c| c.label == label) {
            return f;
        }
    }
    panic!("no header {label}")
}

fn reg(ds: &mut Dataset, ind: &[&str]) -> Vec<OutputItem> {
    let opt = RegressionOptions {
        dependent: ds.find("y").unwrap(),
        independents: ind.iter().map(|n| ds.find(n).unwrap()).collect(),
        collinearity: true,
        durbin_watson: true,
        plot_hist: true,
        plot_pp: true,
        plot_zpred_sresid: true,
        save: SaveFlags::default(),
    };
    run_regression(ds, &opt)
}

fn charts(items: &[OutputItem]) -> Vec<&Chart> {
    items.iter().filter_map(|i| if let OutputItem::Chart(c) = i { Some(c) } else { None }).collect()
}

#[test]
fn regression_title_markers() {
    let mut ds = load("skripsi60.csv");
    let items = reg(&mut ds, &["x1", "x2", "x3"]);
    let tf = |t: &str| table(&items, t).title_footnotes.clone();
    assert_eq!(tf("Variables Entered/Removed"), ['a']);
    assert_eq!(tf("Model Summary"), ['b']);
    assert_eq!(tf("ANOVA"), ['a']);
    assert_eq!(tf("Coefficients"), ['a']);
    assert_eq!(tf("Collinearity Diagnostics"), ['a']);
    assert_eq!(tf("Residuals Statistics"), ['a']);
    // Markers that point at cells/rows stay there; every listed footnote has a marker somewhere.
    let anova = table(&items, "ANOVA");
    assert!(find(&anova.row_headers, "Regression").footnotes.is_empty());
    assert_eq!(anova.cells[0][4].footnotes, ['b']);
    assert_eq!(anova.cells.iter().flatten().filter(|c| !c.footnotes.is_empty()).count(), 1);
    assert_eq!(table(&items, "Model Summary").cells[0][0].footnotes, ['a']);
}

#[test]
fn excluded_variables_title_marker() {
    let mut ds = load("edge_cases.csv");
    let items = reg(&mut ds, &["x1", "x1dbl"]);
    assert_eq!(table(&items, "Excluded Variables").title_footnotes, ['a']);
}

#[test]
fn regression_charts_have_subtitles_not_newlines() {
    let mut ds = load("skripsi60.csv");
    let items = reg(&mut ds, &["x1", "x2", "x3"]);
    let cs = charts(&items);
    assert_eq!(cs.len(), 3);
    for c in cs {
        assert!(!c.title().contains('\n'), "{}", c.title());
        assert_eq!(c.subtitle(), Some("Dependent Variable: y"));
    }
}

#[test]
fn frequencies_histogram_subtitle() {
    let ds = load("skripsi60.csv");
    let items = run_frequencies(&ds, &FrequenciesOptions { vars: vec![ds.find("x1").unwrap()], histogram: true });
    let cs = charts(&items);
    assert_eq!(cs.len(), 1);
    assert_eq!(cs[0].title(), "Histogram");
    assert_eq!(cs[0].subtitle(), Some("x1"));
}

#[test]
fn explore_header_marker_and_qq_title() {
    let ds = load("skripsi60.csv");
    let opt = ExploreOptions { dependents: vec![ds.find("x1").unwrap()], normality: true, qq_plot: true };
    let items = run_explore(&ds, &opt);
    let t = table(&items, "Tests of Normality");
    assert_eq!(find(&t.col_headers, "Kolmogorov-Smirnov").footnotes, ['a']);
    assert!(find(&t.col_headers, "Shapiro-Wilk").footnotes.is_empty());
    for c in charts(&items) {
        assert!(!c.title().contains('\n'));
    }
}

#[test]
fn npar_ks_markers_per_variable() {
    let ds = load("skripsi60.csv");
    let vars: Vec<_> = ["x1", "x2", "x3", "y"].iter().map(|n| ds.find(n).unwrap()).collect();
    let items = run_npar_ks(&ds, &NparKsOptions { vars });
    let t = table(&items, "One-Sample Kolmogorov-Smirnov Test");
    assert_eq!(find(&t.row_headers, "Normal Parameters").footnotes, ['a', 'b']);
    assert!(find(&t.row_headers, "Test Statistic").footnotes.is_empty());
    assert!(find(&t.row_headers, "Asymp. Sig. (2-tailed)").footnotes.is_empty());
    let sig = t.cells.last().unwrap();
    let text: Vec<String> = sig.iter().map(format_cell).collect();
    // Every Sig. carries c; d only where the value is the .200 lower bound.
    for (c, s) in sig.iter().zip(&text) {
        let bound = s.starts_with(".200");
        assert_eq!(c.footnotes, if bound { vec!['c', 'd'] } else { vec!['c'] }, "{s}");
    }
    assert!(text.iter().any(|s| s.ends_with("c,d")) && text.iter().any(|s| s.ends_with('c')), "need a mixed case: {text:?}");
    assert!(t.cells[..t.cells.len() - 1].iter().flatten().all(|c| c.footnotes.is_empty()));
}

#[test]
fn runs_test_value_header_marker() {
    let ds = load("skripsi60.csv");
    let items = run_runs(&ds, &RunsOptions { vars: vec![ds.find("x1").unwrap()] });
    let t = table(&items, "Runs Test");
    assert_eq!(find(&t.row_headers, "Test Value").footnotes, ['a']);
    assert!(t.cells.iter().flatten().all(|c| c.footnotes.is_empty()));
}
