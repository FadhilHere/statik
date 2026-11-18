//! One-Sample Kolmogorov-Smirnov test for normality (SPSS NPAR TESTS /K-S(NORMAL), Lilliefors corrected).
use crate::cells::{LOWER_BOUND_NOTE, fixed_plus, foot, group, label, leaf, num, warning};
use dataset::{Dataset, VarId, VarType};
use output_model::{CellFormat, OutputItem, Table};
use stat_core::lilliefors;

#[derive(Debug, Clone, PartialEq)]
pub struct NparKsOptions {
    pub vars: Vec<VarId>,
}

/// Run the test; missing values are excluded test by test (SPSS default).
pub fn run_npar_ks(ds: &Dataset, opt: &NparKsOptions) -> Vec<OutputItem> {
    let mut out = vec![OutputItem::Title("NPar Tests".into())];
    let mut col_headers = vec![];
    let mut cols = vec![];
    let mut any_bound = false;
    for &id in &opt.vars {
        let var = ds.var(id);
        if !matches!(var.var_type, VarType::Numeric) {
            out.extend(warning(format!("NPar Tests: variable {} is not numeric. It has been omitted.", var.name)));
            continue;
        }
        let x: Vec<f64> = (0..ds.n_cases()).filter(|&r| !ds.is_missing(id, r)).map(|r| ds.numeric(id)[r]).collect();
        let Some(ks) = lilliefors(&x) else {
            let why = if x.len() < 5 {
                format!("has fewer than 5 valid cases ({})", x.len())
            } else if x.iter().all(|&v| v == x[0]) {
                "is constant (standard deviation is zero)".to_string()
            } else {
                "has a standard deviation that is not finite".to_string()
            };
            out.extend(warning(format!(
                "NPar Tests: variable {} {why}. The Kolmogorov-Smirnov test cannot be computed; it has been omitted.",
                var.name
            )));
            continue;
        };
        let n = x.len() as f64;
        let mean = x.iter().sum::<f64>() / n;
        let sd = (x.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>() / (n - 1.0)).sqrt();
        let f3 = CellFormat::Fixed(3);
        let mut sig = foot(num(ks.sig, CellFormat::Sig), 'c');
        if ks.sig_is_lower_bound {
            sig = foot(sig, 'd');
            any_bound = true;
        }
        cols.push(vec![
            num(n, CellFormat::Count),
            num(mean, fixed_plus(var.decimals, 2)),
            num(sd, fixed_plus(var.decimals, 3)),
            num(ks.d, f3),
            num(ks.d_pos, f3),
            num(ks.d_neg, f3),
            num(ks.d, f3),
            sig,
        ]);
        col_headers.push(leaf(&label(ds, id)));
    }
    if cols.is_empty() {
        return out;
    }
    let mut footnotes = vec![
        ('a', "Test distribution is Normal.".to_string()),
        ('b', "Calculated from data.".to_string()),
        ('c', "Lilliefors Corrected.".to_string()),
    ];
    if any_bound {
        footnotes.push(('d', LOWER_BOUND_NOTE.to_string()));
    }
    let cells = (0..cols[0].len()).map(|r| cols.iter().map(|c| c[r].clone()).collect()).collect();
    out.push(OutputItem::Table(Table {
        title: "One-Sample Kolmogorov-Smirnov Test".into(),
        col_headers,
        row_headers: vec![
            leaf("N"),
            group("Normal Parameters", &["Mean", "Std. Deviation"]),
            group("Most Extreme Differences", &["Absolute", "Positive", "Negative"]),
            leaf("Test Statistic"),
            leaf("Asymp. Sig. (2-tailed)"),
        ],
        cells,
        footnotes,
    }));
    out
}
