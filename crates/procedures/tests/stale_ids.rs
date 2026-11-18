use dataset::*;
use output_model::OutputItem;
use procedures::descriptives::{DescriptivesOptions, run_descriptives};
use procedures::explore::{ExploreOptions, run_explore};
use procedures::frequencies::{FrequenciesOptions, run_frequencies};
use procedures::npar_ks::{NparKsOptions, run_npar_ks};
use procedures::npar_runs::{RunsOptions, run_runs};
use procedures::oneway::{OneWayOptions, run_oneway};
use procedures::regression::{RegressionOptions, SaveFlags, run_regression};
use procedures::ttest_ind::{IndependentOptions, run_independent};
use procedures::ttest_one::{OneSampleOptions, run_one_sample};
use procedures::ttest_paired::{PairedOptions, run_paired};

const BAD: VarId = VarId(999);
const X: VarId = VarId(0);
const S: VarId = VarId(1);

fn ds() -> Dataset {
    let mut d = Dataset::new();
    d.add_variable(Variable::numeric("x"), Column::Numeric(vec![1.0, 2.0, 3.0, 4.0, 5.0, 7.0])).unwrap();
    d.add_variable(Variable::string("s", 8), Column::Text(vec!["a".into(); 6])).unwrap();
    d
}

fn one_warning(items: &[OutputItem], needle: &str) {
    assert_eq!(items.len(), 1, "{items:?}");
    match &items[0] {
        OutputItem::Warning(w) => assert!(w.contains(needle), "{w}"),
        other => panic!("expected warning, got {other:?}"),
    }
}

#[test]
fn stale_ids_give_one_warning() {
    let mut d = ds();
    let gone = "no longer exists (index 999)";
    one_warning(
        &run_descriptives(&mut d, &DescriptivesOptions { vars: vec![X, BAD], skew_kurt: false, save_z: false }),
        gone,
    );
    one_warning(&run_explore(&d, &ExploreOptions { dependents: vec![BAD], normality: false, qq_plot: false }), gone);
    one_warning(&run_frequencies(&d, &FrequenciesOptions { vars: vec![BAD], histogram: false }), gone);
    one_warning(&run_npar_ks(&d, &NparKsOptions { vars: vec![BAD] }), gone);
    one_warning(&run_runs(&d, &RunsOptions { vars: vec![BAD] }), gone);
    one_warning(
        &run_oneway(&d, &OneWayOptions { dependent: X, factor: BAD, descriptives: false, homogeneity: false }),
        gone,
    );
    let reg = RegressionOptions {
        dependent: X,
        independents: vec![BAD],
        collinearity: false,
        durbin_watson: false,
        plot_hist: false,
        plot_pp: false,
        plot_zpred_sresid: false,
        save: SaveFlags::default(),
    };
    one_warning(&run_regression(&mut d, &reg), gone);
    one_warning(
        &run_independent(
            &d,
            &IndependentOptions { test_vars: vec![X], grouping: BAD, group1: 1.0, group2: 2.0, conf: 0.95 },
        ),
        gone,
    );
    one_warning(&run_one_sample(&d, &OneSampleOptions { vars: vec![BAD], test_value: 0.0, conf: 0.95 }), gone);
    one_warning(&run_paired(&d, &PairedOptions { pairs: vec![(X, BAD)], conf: 0.95 }), gone);
}

#[test]
fn text_variable_where_numeric_required() {
    let mut d = ds();
    one_warning(
        &run_oneway(&d, &OneWayOptions { dependent: X, factor: S, descriptives: false, homogeneity: false }),
        "variable s is not numeric",
    );
    let reg = RegressionOptions {
        dependent: S,
        independents: vec![X],
        collinearity: false,
        durbin_watson: false,
        plot_hist: false,
        plot_pp: false,
        plot_zpred_sresid: false,
        save: SaveFlags::default(),
    };
    one_warning(&run_regression(&mut d, &reg), "variable s is not numeric");
    one_warning(
        &run_independent(
            &d,
            &IndependentOptions { test_vars: vec![X], grouping: S, group1: 1.0, group2: 2.0, conf: 0.95 },
        ),
        "variable s is not numeric",
    );
}
