//! Explore (SPSS EXAMINE): Case Processing Summary, Descriptives, Tests of Normality, Normal Q-Q plots.
use crate::cells::{LOWER_BOUND_NOTE, check_ids, empty, fixed_plus, foot, label, leaf, num, text, warning};
use dataset::{Dataset, VarId, VarType};
use output_model::{Cell, CellFormat, Chart, HeaderNode, OutputItem, Table};
use stat_core::{describe, lilliefors, normal_quantile, percentile_haverage, shapiro_wilk, t_quantile, trimmed_mean_5};

#[derive(Debug, Clone, PartialEq)]
pub struct ExploreOptions {
    pub dependents: Vec<VarId>,
    /// Tests of Normality table.
    pub normality: bool,
    /// Normal Q-Q plot per dependent.
    pub qq_plot: bool,
}

const F3: CellFormat = CellFormat::Fixed(3);

fn percent(count: usize, total: usize) -> Cell {
    if total == 0 {
        return text(".");
    }
    let p = (count as f64 * 100.0 / total as f64 * 10.0).round() / 10.0;
    text(&format!("{p:.1}%"))
}

fn count(n: usize) -> Cell {
    num(n as f64, CellFormat::Count)
}

fn is_constant(x: &[f64]) -> bool {
    x.iter().all(|&v| v == x[0])
}

/// Run Explore with listwise exclusion over the dependents (SPSS EXAMINE default).
pub fn run_explore(ds: &Dataset, opt: &ExploreOptions) -> Vec<OutputItem> {
    if let Err(w) = check_ids(ds, &opt.dependents, "Explore") {
        return w;
    }
    let mut out = vec![OutputItem::Title("Explore".into())];
    let mut deps = vec![];
    for &id in &opt.dependents {
        if matches!(ds.var(id).var_type, VarType::Numeric) {
            deps.push(id);
        } else {
            let name = &ds.var(id).name;
            out.push(OutputItem::Warning(format!("Explore: variable {name} is not numeric. It has been omitted.")));
        }
    }
    if deps.is_empty() {
        out.extend(warning("Explore: no numeric dependent variables were specified. No analysis was performed."));
        return out;
    }
    let rows = ds.listwise_rows(&deps);
    let labels: Vec<String> = deps.iter().map(|&id| label(ds, id)).collect();
    out.push(OutputItem::Table(case_summary(&labels, rows.len(), ds.n_cases())));
    if rows.is_empty() {
        out.push(OutputItem::Warning("Explore: there are no valid cases. No statistics are computed.".into()));
        return out;
    }
    let data: Vec<Vec<f64>> = deps
        .iter()
        .map(|&id| {
            let mut v: Vec<f64> = rows.iter().map(|&r| ds.numeric(id)[r]).collect();
            v.sort_by(f64::total_cmp);
            v
        })
        .collect();
    let decimals: Vec<usize> = deps.iter().map(|&id| ds.var(id).decimals).collect();
    out.push(OutputItem::Table(descriptives(&labels, &data, &decimals)));

    let mut normal_rows = vec![];
    for (j, x) in data.iter().enumerate() {
        if is_constant(x) {
            out.push(OutputItem::Warning(format!(
                "Explore: {} is constant. It has been omitted from the normality tests and plots.",
                labels[j]
            )));
        } else {
            normal_rows.push(j);
        }
    }
    if opt.normality && !normal_rows.is_empty() {
        let (table, notes) = normality_table(&labels, &data, &normal_rows);
        out.push(OutputItem::Table(table));
        out.extend(notes.into_iter().map(OutputItem::Warning));
    }
    if opt.qq_plot {
        for &j in &normal_rows {
            out.push(OutputItem::Chart(Chart::QQPlot {
                title: format!("Normal Q-Q Plot of {}", labels[j]),
                subtitle: None,
                observed: data[j].clone(),
                expected: blom_expected(&data[j]),
            }));
        }
    }
    out
}

fn case_summary(labels: &[String], valid: usize, total: usize) -> Table {
    let missing = total - valid;
    let pair = |s: &str| HeaderNode { label: s.into(), footnotes: vec![], children: vec![leaf("N"), leaf("Percent")] };
    Table {
        title: "Case Processing Summary".into(),
        title_footnotes: vec![],
        col_headers: vec![HeaderNode {
            label: "Cases".into(),
            footnotes: vec![],
            children: vec![pair("Valid"), pair("Missing"), pair("Total")],
        }],
        row_headers: labels.iter().map(|l| leaf(l)).collect(),
        cells: labels
            .iter()
            .map(|_| {
                vec![
                    count(valid),
                    percent(valid, total),
                    count(missing),
                    percent(missing, total),
                    count(total),
                    percent(total, total),
                ]
            })
            .collect(),
        footnotes: vec![],
    }
}

/// Decimals (unverified against SPSS, see KNOWN_GAPS.md): Mean, CI, 5% Trimmed Mean, Median,
/// Interquartile Range with variable decimals + 2; Std. Error of Mean, Variance, Std. Deviation + 3;
/// Minimum, Maximum, Range with the variable's decimals; Skewness, Kurtosis and their SEs `Fixed(3)`.
fn descriptives(labels: &[String], data: &[Vec<f64>], decimals: &[usize]) -> Table {
    let mut row_headers = vec![];
    let mut cells = vec![];
    for (j, x) in data.iter().enumerate() {
        let d = describe(x).expect("non-empty");
        let (dp, d2, d3) = (fixed_plus(decimals[j], 0), fixed_plus(decimals[j], 2), fixed_plus(decimals[j], 3));
        let n = x.len() as f64;
        let half = t_quantile(0.975, n - 1.0) * d.se_mean;
        let nan = f64::NAN;
        let stat_se = |s: f64, fs: CellFormat, se: f64, fse: CellFormat| vec![num(s, fs), num(se, fse)];
        let only = |s: f64, fs: CellFormat| vec![num(s, fs), empty()];
        cells.push(stat_se(d.mean, d2, d.se_mean, d3));
        cells.push(only(d.mean - half, d2));
        cells.push(only(d.mean + half, d2));
        cells.push(only(trimmed_mean_5(x), d2));
        cells.push(only(percentile_haverage(x, 50.0), d2));
        cells.push(only(d.variance, d3));
        cells.push(only(d.sd, d3));
        cells.push(only(d.min, dp));
        cells.push(only(d.max, dp));
        cells.push(only(d.range, dp));
        cells.push(only(percentile_haverage(x, 75.0) - percentile_haverage(x, 25.0), d2));
        cells.push(stat_se(d.skewness.unwrap_or(nan), F3, d.se_skewness.unwrap_or(nan), F3));
        cells.push(stat_se(d.kurtosis.unwrap_or(nan), F3, d.se_kurtosis.unwrap_or(nan), F3));
        let ci = HeaderNode {
            label: "95% Confidence Interval for Mean".into(),
            footnotes: vec![],
            children: vec![leaf("Lower Bound"), leaf("Upper Bound")],
        };
        let mut children = vec![leaf("Mean"), ci];
        children.extend(
            [
                "5% Trimmed Mean",
                "Median",
                "Variance",
                "Std. Deviation",
                "Minimum",
                "Maximum",
                "Range",
                "Interquartile Range",
                "Skewness",
                "Kurtosis",
            ]
            .map(leaf),
        );
        row_headers.push(HeaderNode { label: labels[j].clone(), footnotes: vec![], children });
    }
    Table {
        title: "Descriptives".into(),
        title_footnotes: vec![],
        col_headers: vec![leaf("Statistic"), leaf("Std. Error")],
        row_headers,
        cells,
        footnotes: vec![],
    }
}

/// Tests of Normality for the rows `which`; returns the table and warnings for tests that could
/// not be computed.
fn normality_table(labels: &[String], data: &[Vec<f64>], which: &[usize]) -> (Table, Vec<String>) {
    let mut cells = vec![];
    let mut notes = vec![];
    let mut any_bound = false;
    for &j in which {
        let x = &data[j];
        let n = x.len();
        let mut row = match lilliefors(x) {
            Some(ks) => {
                let mut sig = num(ks.sig, CellFormat::Sig);
                if ks.sig_is_lower_bound {
                    sig = foot(sig, '*');
                    any_bound = true;
                }
                vec![num(ks.d, F3), count(n), sig]
            }
            None => {
                notes.push(format!(
                    "Explore: the Kolmogorov-Smirnov (Lilliefors) test for {} needs at least 5 valid cases.",
                    labels[j]
                ));
                vec![text("."), count(n), text(".")]
            }
        };
        match shapiro_wilk(x) {
            Some(sw) => row.extend([num(sw.w, F3), count(n), num(sw.sig, CellFormat::Sig)]),
            None => {
                notes.push(format!(
                    "Explore: the Shapiro-Wilk test for {} needs between 3 and 5000 valid cases.",
                    labels[j]
                ));
                row.extend([text("."), count(n), text(".")]);
            }
        }
        cells.push(row);
    }
    let mut footnotes = vec![];
    if any_bound {
        footnotes.push(('*', LOWER_BOUND_NOTE.to_string()));
    }
    footnotes.push(('a', "Lilliefors Significance Correction".to_string()));
    let triple = |s: &str, marks: &[char]| HeaderNode {
        label: s.into(),
        footnotes: marks.to_vec(),
        children: vec![leaf("Statistic"), leaf("df"), leaf("Sig.")],
    };
    let table = Table {
        title: "Tests of Normality".into(),
        title_footnotes: vec![],
        col_headers: vec![triple("Kolmogorov-Smirnov", &['a']), triple("Shapiro-Wilk", &[])],
        row_headers: which.iter().map(|&j| leaf(&labels[j])).collect(),
        cells,
        footnotes,
    };
    (table, notes)
}

/// Expected standard-normal values for an ascending sample with Blom's proportions
/// (r − 3/8)/(n + 1/4); tied values share their mean rank.
fn blom_expected(sorted: &[f64]) -> Vec<f64> {
    let n = sorted.len();
    let mut out = vec![0.0; n];
    let mut i = 0;
    while i < n {
        let mut k = i;
        while k + 1 < n && sorted[k + 1] == sorted[i] {
            k += 1;
        }
        let rank = (i + k) as f64 / 2.0 + 1.0;
        let z = normal_quantile((rank - 0.375) / (n as f64 + 0.25));
        out[i..=k].fill(z);
        i = k + 1;
    }
    out
}
