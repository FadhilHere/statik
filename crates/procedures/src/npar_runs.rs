//! Runs test about the median (SPSS NPAR TESTS /RUNS(MEDIAN)).
use crate::cells::{check_ids, fixed_plus, foot, label, leaf, num, warning};
use dataset::{Dataset, VarId, VarType};
use output_model::{CellFormat, OutputItem, Table};
use stat_core::runs_test_median;

#[derive(Debug, Clone, PartialEq)]
pub struct RunsOptions {
    pub vars: Vec<VarId>,
}

/// Run the test; missing values are excluded test by test (SPSS default).
pub fn run_runs(ds: &Dataset, o: &RunsOptions) -> Vec<OutputItem> {
    if let Err(w) = check_ids(ds, &o.vars, "NPar Tests") {
        return w;
    }
    let mut out = vec![OutputItem::Title("NPar Tests".into())];
    let mut col_headers = vec![];
    let mut cols = vec![];
    for &id in &o.vars {
        let var = ds.var(id);
        if !matches!(var.var_type, VarType::Numeric) {
            out.extend(warning(format!("NPar Tests: variable {} is not numeric. It has been omitted.", var.name)));
            continue;
        }
        let x: Vec<f64> = (0..ds.n_cases()).filter(|&r| !ds.is_missing(id, r)).map(|r| ds.numeric(id)[r]).collect();
        let Some(r) = runs_test_median(&x) else {
            out.extend(warning(format!(
                "NPar Tests: variable {} has all cases on one side of the test value (or fewer than 2 valid cases). \
                 The Runs test cannot be computed; it has been omitted.",
                var.name
            )));
            continue;
        };
        let count = |v: usize| num(v as f64, CellFormat::Count);
        cols.push(vec![
            foot(num(r.test_value, fixed_plus(var.decimals, 2)), 'a'),
            count(r.n_below),
            count(r.n_at_or_above),
            count(r.n_below + r.n_at_or_above),
            count(r.runs),
            num(r.z, CellFormat::Fixed(3)),
            num(r.sig, CellFormat::Sig),
        ]);
        col_headers.push(leaf(&label(ds, id)));
    }
    if cols.is_empty() {
        return out;
    }
    let cells = (0..cols[0].len()).map(|r| cols.iter().map(|c| c[r].clone()).collect()).collect();
    out.push(OutputItem::Table(Table {
        title: "Runs Test".into(),
        col_headers,
        row_headers: [
            "Test Value",
            "Cases < Test Value",
            "Cases >= Test Value",
            "Total Cases",
            "Number of Runs",
            "Z",
            "Asymp. Sig. (2-tailed)",
        ]
        .map(leaf)
        .to_vec(),
        cells,
        footnotes: vec![('a', "Median".to_string())],
    }));
    out
}
