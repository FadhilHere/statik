//! Independent-Samples T Test (SPSS T-TEST GROUPS).
use crate::cells::{
    SD_ZERO_MSG, df_cell, empty, fixed_plus, group, label, leaf, num, parent, sig_header, text, warning,
};
use crate::ttest_one::pct;
use dataset::{Dataset, OrderedValue, VarId, VarType};
use output_model::{Cell, CellFormat, OutputItem, Table};
use stat_core::{TResult, describe, independent_t};

#[derive(Debug, Clone, PartialEq)]
pub struct IndependentOptions {
    pub test_vars: Vec<VarId>,
    pub grouping: VarId,
    pub group1: f64,
    pub group2: f64,
    pub conf: f64,
}

fn test_row(t: &TResult, decimals: usize, levene: Option<(f64, f64)>) -> Vec<Cell> {
    // The Welch row (no Levene cells) always shows df with 3 decimals, even when it is integral.
    let df = if levene.is_some() { df_cell(t.df) } else { num(t.df, CellFormat::Fixed(3)) };
    let (f, s) = match levene {
        Some((f, s)) => (num(f, CellFormat::Fixed(3)), num(s, CellFormat::Sig)),
        None => (empty(), empty()),
    };
    vec![
        f,
        s,
        num(t.t, CellFormat::Fixed(3)),
        df,
        num(t.sig / 2.0, CellFormat::Sig),
        num(t.sig, CellFormat::Sig),
        num(t.mean_diff, fixed_plus(decimals, 2)),
        num(t.se_diff, fixed_plus(decimals, 3)),
        num(t.ci.0, fixed_plus(decimals, 2)),
        num(t.ci.1, fixed_plus(decimals, 2)),
    ]
}

/// Run the test; a case is used for a test variable when that variable is valid and the grouping
/// value is one of the two groups (analysis-by-analysis exclusion).
pub fn run_independent(ds: &Dataset, o: &IndependentOptions) -> Vec<OutputItem> {
    let mut out = vec![OutputItem::Title("T-Test".into())];
    let g = ds.var(o.grouping);
    if !matches!(g.var_type, VarType::Numeric) {
        out.extend(warning(format!("T-Test: grouping variable {} is not numeric.", g.name)));
        return out;
    }
    let glabel =
        |v: f64| g.value_labels.get(&OrderedValue(v)).cloned().unwrap_or_else(|| format!("{:.*}", g.decimals, v + 0.0));
    let (l1, l2) = (glabel(o.group1), glabel(o.group2));
    let (mut stat_h, mut stat_rows) = (vec![], vec![]);
    let (mut test_h, mut test_rows) = (vec![], vec![]);
    let mut warns = vec![];
    for &id in &o.test_vars {
        let var = ds.var(id);
        if !matches!(var.var_type, VarType::Numeric) {
            warns.extend(warning(format!("T-Test: variable {} is not numeric. It has been omitted.", var.name)));
            continue;
        }
        let (mut a, mut b) = (vec![], vec![]);
        for r in 0..ds.n_cases() {
            if ds.is_missing(id, r) || ds.is_missing(o.grouping, r) {
                continue;
            }
            let gv = ds.numeric(o.grouping)[r];
            if gv == o.group1 {
                a.push(ds.numeric(id)[r]);
            } else if gv == o.group2 {
                b.push(ds.numeric(id)[r]);
            }
        }
        let name = label(ds, id);
        stat_h.push(group(&name, &[&l1, &l2]));
        for x in [&a, &b] {
            match describe(x) {
                Some(d) => stat_rows.push(vec![
                    num(d.n as f64, CellFormat::Count),
                    num(d.mean, fixed_plus(var.decimals, 2)),
                    num(d.sd, fixed_plus(var.decimals, 3)),
                    num(d.se_mean, fixed_plus(var.decimals, 3)),
                ]),
                None => stat_rows.push(vec![num(0.0, CellFormat::Count), text("."), text("."), text(".")]),
            }
        }
        let Some(r) = independent_t(&a, &b, o.conf) else {
            let why = if a.is_empty() || b.is_empty() {
                let missing = if a.is_empty() { &l1 } else { &l2 };
                format!(
                    "T-Test: variable {} has no valid cases in group {missing} of {}. The test cannot be computed.",
                    var.name, g.name
                )
            } else if a.len() < 2 || b.len() < 2 {
                format!(
                    "T-Test: variable {} has fewer than 2 valid cases in a group. The test cannot be computed.",
                    var.name
                )
            } else {
                format!("{SD_ZERO_MSG} (variable {})", var.name)
            };
            warns.extend(warning(why));
            continue;
        };
        test_h.push(group(&name, &["Equal variances assumed", "Equal variances not assumed"]));
        test_rows.push(test_row(&r.equal, var.decimals, Some((r.levene_f, r.levene_sig))));
        test_rows.push(test_row(&r.welch, var.decimals, None));
    }
    if !stat_rows.is_empty() {
        out.push(OutputItem::Table(Table {
            title: "Group Statistics".into(),
            col_headers: ["N", "Mean", "Std. Deviation", "Std. Error Mean"].iter().map(|s| leaf(s)).collect(),
            row_headers: stat_h,
            cells: stat_rows,
            footnotes: vec![],
        }));
    }
    out.extend(warns);
    if !test_rows.is_empty() {
        let ci = format!("{}% Confidence Interval of the Difference", pct(o.conf));
        out.push(OutputItem::Table(Table {
            title: "Independent Samples Test".into(),
            col_headers: vec![
                group("Levene's Test for Equality of Variances", &["F", "Sig."]),
                parent(
                    "t-test for Equality of Means",
                    vec![
                        leaf("t"),
                        leaf("df"),
                        sig_header(),
                        leaf("Mean Difference"),
                        leaf("Std. Error Difference"),
                        group(&ci, &["Lower", "Upper"]),
                    ],
                ),
            ],
            row_headers: test_h,
            cells: test_rows,
            footnotes: vec![],
        }));
    }
    out
}
