//! Paired-Samples T Test (SPSS T-TEST PAIRS).
use crate::cells::{SD_ZERO_MSG, check_ids, df_cell, fixed_plus, group, label, leaf, num, parent, sig_header, warning};
use crate::ttest_one::pct;
use dataset::{Dataset, VarId, VarType};
use output_model::{Cell, CellFormat, OutputItem, Table};
use stat_core::{describe, paired_corr, paired_t};

#[derive(Debug, Clone, PartialEq)]
pub struct PairedOptions {
    pub pairs: Vec<(VarId, VarId)>,
    pub conf: f64,
}

fn corr_row(n: usize, c: f64, sig: f64) -> Vec<Cell> {
    vec![
        num(n as f64, CellFormat::Count),
        num(c, CellFormat::Fixed(3)),
        num(sig / 2.0, CellFormat::Sig),
        num(sig, CellFormat::Sig),
    ]
}

/// Run the test; each pair uses its own complete cases (per-pair listwise).
pub fn run_paired(ds: &Dataset, o: &PairedOptions) -> Vec<OutputItem> {
    let pair_ids: Vec<VarId> = o.pairs.iter().flat_map(|&(a, b)| [a, b]).collect();
    if let Err(w) = check_ids(ds, &pair_ids, "T-Test") {
        return w;
    }
    let mut out = vec![OutputItem::Title("T-Test".into())];
    let (mut stat_h, mut stat_rows) = (vec![], vec![]);
    let (mut corr_h, mut corr_rows) = (vec![], vec![]);
    let (mut test_h, mut test_rows) = (vec![], vec![]);
    let mut warns = vec![];
    for (i, &(a, b)) in o.pairs.iter().enumerate() {
        let (va, vb) = (ds.var(a), ds.var(b));
        let k = i + 1;
        if !matches!(va.var_type, VarType::Numeric) || !matches!(vb.var_type, VarType::Numeric) {
            warns.extend(warning(format!("T-Test: Pair {k} contains a non-numeric variable. It has been omitted.")));
            continue;
        }
        let rows: Vec<usize> = (0..ds.n_cases()).filter(|&r| !ds.is_missing(a, r) && !ds.is_missing(b, r)).collect();
        let xa: Vec<f64> = rows.iter().map(|&r| ds.numeric(a)[r]).collect();
        let xb: Vec<f64> = rows.iter().map(|&r| ds.numeric(b)[r]).collect();
        let (Some(da), Some(db)) = (describe(&xa), describe(&xb)) else {
            warns.extend(warning(format!("T-Test: Pair {k} has no valid cases. It has been omitted.")));
            continue;
        };
        let (la, lb) = (label(ds, a), label(ds, b));
        stat_h.push(group(&format!("Pair {k}"), &[&la, &lb]));
        for (d, v) in [(&da, va), (&db, vb)] {
            stat_rows.push(vec![
                num(d.mean, fixed_plus(v.decimals, 2)),
                num(d.n as f64, CellFormat::Count),
                num(d.sd, fixed_plus(v.decimals, 3)),
                num(d.se_mean, fixed_plus(v.decimals, 3)),
            ]);
        }
        let Some(r) = paired_t(&xa, &xb, o.conf) else {
            let why = if rows.len() < 2 {
                "t cannot be computed because there are fewer than 2 valid cases."
            } else {
                SD_ZERO_MSG
            };
            warns.extend(warning(why));
            // SPSS still prints the correlation when only the differences are constant.
            let (c, cs) = paired_corr(&xa, &xb);
            corr_h.push(group(&format!("Pair {k}"), &[&format!("{la} & {lb}")]));
            corr_rows.push(corr_row(rows.len(), c, cs));
            continue;
        };
        let dec = va.decimals.max(vb.decimals);
        corr_h.push(group(&format!("Pair {k}"), &[&format!("{la} & {lb}")]));
        corr_rows.push(corr_row(rows.len(), r.corr, r.corr_sig));
        test_h.push(group(&format!("Pair {k}"), &[&format!("{la} - {lb}")]));
        test_rows.push(vec![
            num(r.test.mean_diff, fixed_plus(dec, 2)),
            num(r.sd_diff, fixed_plus(dec, 3)),
            num(r.test.se_diff, fixed_plus(dec, 3)),
            num(r.test.ci.0, fixed_plus(dec, 2)),
            num(r.test.ci.1, fixed_plus(dec, 2)),
            num(r.test.t, CellFormat::Fixed(3)),
            df_cell(r.test.df),
            num(r.test.sig / 2.0, CellFormat::Sig),
            num(r.test.sig, CellFormat::Sig),
        ]);
    }
    if stat_rows.is_empty() {
        out.extend(warns);
        return out;
    }
    out.push(OutputItem::Table(Table {
        title: "Paired Samples Statistics".into(),
        title_footnotes: vec![],
        col_headers: ["Mean", "N", "Std. Deviation", "Std. Error Mean"].iter().map(|s| leaf(s)).collect(),
        row_headers: stat_h,
        cells: stat_rows,
        footnotes: vec![],
    }));
    out.extend(warns);
    if !corr_rows.is_empty() {
        out.push(OutputItem::Table(Table {
            title: "Paired Samples Correlations".into(),
            title_footnotes: vec![],
            col_headers: vec![leaf("N"), leaf("Correlation"), sig_header()],
            row_headers: corr_h,
            cells: corr_rows,
            footnotes: vec![],
        }));
    }
    if !test_rows.is_empty() {
        let ci = format!("{}% Confidence Interval of the Difference", pct(o.conf));
        out.push(OutputItem::Table(Table {
            title: "Paired Samples Test".into(),
            title_footnotes: vec![],
            col_headers: vec![
                parent(
                    "Paired Differences",
                    vec![
                        leaf("Mean"),
                        leaf("Std. Deviation"),
                        leaf("Std. Error Mean"),
                        group(&ci, &["Lower", "Upper"]),
                    ],
                ),
                leaf("t"),
                leaf("df"),
                sig_header(),
            ],
            row_headers: test_h,
            cells: test_rows,
            footnotes: vec![],
        }));
    }
    out
}
