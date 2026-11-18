//! One-Sample T Test (SPSS T-TEST /TESTVAL).
use crate::cells::{SD_ZERO_MSG, check_ids, df_cell, fixed_plus, group, label, leaf, num, parent, sig_header, warning};
use dataset::{Dataset, VarId, VarType};
use output_model::{CellFormat, OutputItem, Table};
use stat_core::{describe, one_sample_t};

#[derive(Debug, Clone, PartialEq)]
pub struct OneSampleOptions {
    pub vars: Vec<VarId>,
    pub test_value: f64,
    pub conf: f64,
}

pub(crate) fn pct(conf: f64) -> String {
    let p = conf * 100.0;
    if p.fract() == 0.0 { format!("{p:.0}") } else { format!("{p}") }
}

/// Run the test; missing values are excluded variable by variable (SPSS default).
pub fn run_one_sample(ds: &Dataset, o: &OneSampleOptions) -> Vec<OutputItem> {
    if let Err(w) = check_ids(ds, &o.vars, "T-Test") {
        return w;
    }
    let mut out = vec![OutputItem::Title("T-Test".into())];
    let mut stat_rows = vec![];
    let mut test_rows = vec![];
    let mut warns = vec![];
    let (mut stat_h, mut test_h) = (vec![], vec![]);
    for &id in &o.vars {
        let var = ds.var(id);
        if !matches!(var.var_type, VarType::Numeric) {
            warns.extend(warning(format!("T-Test: variable {} is not numeric. It has been omitted.", var.name)));
            continue;
        }
        let x: Vec<f64> = (0..ds.n_cases()).filter(|&r| !ds.is_missing(id, r)).map(|r| ds.numeric(id)[r]).collect();
        let Some(d) = describe(&x) else {
            warns.extend(warning(format!("T-Test: variable {} has no valid cases. It has been omitted.", var.name)));
            continue;
        };
        stat_h.push(leaf(&label(ds, id)));
        stat_rows.push(vec![
            num(d.n as f64, CellFormat::Count),
            num(d.mean, fixed_plus(var.decimals, 2)),
            num(d.sd, fixed_plus(var.decimals, 3)),
            num(d.se_mean, fixed_plus(var.decimals, 3)),
        ]);
        let Some(r) = one_sample_t(&x, o.test_value, o.conf) else {
            let why =
                if d.n < 2 { "t cannot be computed because there are fewer than 2 valid cases." } else { SD_ZERO_MSG };
            warns.extend(warning(why));
            continue;
        };
        test_h.push(leaf(&label(ds, id)));
        test_rows.push(vec![
            num(r.t, CellFormat::Fixed(3)),
            df_cell(r.df),
            num(r.sig / 2.0, CellFormat::Sig),
            num(r.sig, CellFormat::Sig),
            num(r.mean_diff, fixed_plus(var.decimals, 2)),
            num(r.ci.0, fixed_plus(var.decimals, 2)),
            num(r.ci.1, fixed_plus(var.decimals, 2)),
        ]);
    }
    if stat_rows.is_empty() {
        out.extend(warns);
        return out;
    }
    out.push(OutputItem::Table(Table {
        title: "One-Sample Statistics".into(),
        col_headers: ["N", "Mean", "Std. Deviation", "Std. Error Mean"].iter().map(|s| leaf(s)).collect(),
        row_headers: stat_h,
        cells: stat_rows,
        footnotes: vec![],
    }));
    out.extend(warns);
    if !test_rows.is_empty() {
        let tv = format!("Test Value = {}", o.test_value);
        let ci = format!("{}% Confidence Interval of the Difference", pct(o.conf));
        out.push(OutputItem::Table(Table {
            title: "One-Sample Test".into(),
            col_headers: vec![parent(
                &tv,
                vec![leaf("t"), leaf("df"), sig_header(), leaf("Mean Difference"), group(&ci, &["Lower", "Upper"])],
            )],
            row_headers: test_h,
            cells: test_rows,
            footnotes: vec![],
        }));
    }
    out
}
