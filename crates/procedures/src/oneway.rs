//! One-Way ANOVA (SPSS ONEWAY /STATISTICS DESCRIPTIVES HOMOGENEITY).
use crate::cells::{empty, fixed_plus, group, label, leaf, num, warning};
use crate::ttest_one::pct;
use dataset::{Dataset, OrderedValue, VarId, VarType};
use output_model::{CellFormat, OutputItem, Table};
use stat_core::{describe, levene_mean, oneway, t_quantile};

#[derive(Debug, Clone, PartialEq)]
pub struct OneWayOptions {
    pub dependent: VarId,
    pub factor: VarId,
    pub descriptives: bool,
    pub homogeneity: bool,
}

/// Run the analysis; cases missing on the dependent or the factor are excluded.
pub fn run_oneway(ds: &Dataset, o: &OneWayOptions) -> Vec<OutputItem> {
    let mut out = vec![OutputItem::Title("Oneway".into())];
    let (dep, fac) = (ds.var(o.dependent), ds.var(o.factor));
    for v in [dep, fac] {
        if !matches!(v.var_type, VarType::Numeric) {
            out.extend(warning(format!("Oneway: variable {} is not numeric.", v.name)));
            return out;
        }
    }
    let mut keys: Vec<f64> = vec![];
    let mut data: Vec<Vec<f64>> = vec![];
    let mut rows: Vec<(f64, f64)> = (0..ds.n_cases())
        .filter(|&r| !ds.is_missing(o.dependent, r) && !ds.is_missing(o.factor, r))
        .map(|r| (ds.numeric(o.factor)[r], ds.numeric(o.dependent)[r]))
        .collect();
    rows.sort_by(|a, b| a.0.total_cmp(&b.0)); // stable: keeps case order inside a group
    for (g, v) in rows {
        if keys.last() != Some(&g) {
            keys.push(g);
            data.push(vec![]);
        }
        data.last_mut().unwrap().push(v);
    }
    if data.len() < 2 {
        out.extend(warning("Oneway: at least two groups with valid data are required. The analysis was not run."));
        return out;
    }
    let slices: Vec<&[f64]> = data.iter().map(|g| g.as_slice()).collect();
    let Some(a) = oneway(&slices) else {
        out.extend(warning(
            "Oneway: there are not enough valid cases to compute the within-groups variance. The analysis was not run.",
        ));
        return out;
    };
    let dec = dep.decimals;
    let glabel =
        |v: f64| fac.value_labels.get(&OrderedValue(v)).cloned().unwrap_or_else(|| format!("{v:.*}", fac.decimals));
    let f3 = CellFormat::Fixed(3);
    let count = |v: usize| num(v as f64, CellFormat::Count);
    let all: Vec<f64> = data.iter().flatten().copied().collect();

    if o.descriptives {
        let ci = format!("{}% Confidence Interval for Mean", pct(0.95));
        let mut names: Vec<String> = keys.iter().map(|&k| glabel(k)).collect();
        names.push("Total".into());
        let mut sets: Vec<&[f64]> = slices.clone();
        sets.push(&all);
        let cells = sets
            .iter()
            .map(|s| {
                let d = describe(s).expect("non-empty group");
                let half = if d.n > 1 { t_quantile(0.975, (d.n - 1) as f64) * d.se_mean } else { f64::NAN };
                vec![
                    count(d.n),
                    num(d.mean, fixed_plus(dec, 2)),
                    num(d.sd, fixed_plus(dec, 3)),
                    num(d.se_mean, fixed_plus(dec, 3)),
                    num(d.mean - half, fixed_plus(dec, 2)),
                    num(d.mean + half, fixed_plus(dec, 2)),
                    num(d.min, fixed_plus(dec, 0)),
                    num(d.max, fixed_plus(dec, 0)),
                ]
            })
            .collect();
        out.push(OutputItem::Table(Table {
            title: "Descriptives".into(),
            col_headers: vec![
                leaf("N"),
                leaf("Mean"),
                leaf("Std. Deviation"),
                leaf("Std. Error"),
                group(&ci, &["Lower Bound", "Upper Bound"]),
                leaf("Minimum"),
                leaf("Maximum"),
            ],
            row_headers: names.iter().map(|s| leaf(s)).collect(),
            cells,
            footnotes: vec![],
        }));
    }
    if o.homogeneity {
        let (f, sig, df1, df2) = levene_mean(&slices);
        out.push(OutputItem::Table(Table {
            title: "Tests of Homogeneity of Variances".into(),
            col_headers: ["Levene Statistic", "df1", "df2", "Sig."].map(leaf).to_vec(),
            row_headers: vec![leaf("Based on Mean")],
            cells: vec![vec![num(f, f3), count(df1), count(df2), num(sig, CellFormat::Sig)]],
            footnotes: vec![],
        }));
    }
    let ms_b = a.ss_between / a.df_between as f64;
    let ms_w = a.ss_within / a.df_within as f64;
    out.push(OutputItem::Table(Table {
        title: "ANOVA".into(),
        col_headers: ["Sum of Squares", "df", "Mean Square", "F", "Sig."].map(leaf).to_vec(),
        row_headers: ["Between Groups", "Within Groups", "Total"].map(leaf).to_vec(),
        cells: vec![
            vec![num(a.ss_between, f3), count(a.df_between), num(ms_b, f3), num(a.f, f3), num(a.sig, CellFormat::Sig)],
            vec![num(a.ss_within, f3), count(a.df_within), num(ms_w, f3), empty(), empty()],
            vec![num(a.ss_between + a.ss_within, f3), count(a.df_between + a.df_within), empty(), empty(), empty()],
        ],
        footnotes: vec![],
    }));
    if a.f.is_nan() {
        out.extend(warning(format!(
            "Oneway: the within-groups sum of squares of {} is 0, so F cannot be computed.",
            label(ds, o.dependent)
        )));
    }
    out
}
