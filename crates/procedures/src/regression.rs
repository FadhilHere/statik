//! Linear Regression (SPSS REGRESSION, method Enter).
use dataset::{Column, Dataset, VarId, VarType, Variable};
use crate::cells::{empty, foot, group, label, leaf, num, num_or_empty, text, warning};
use output_model::{CellFormat, Chart, HeaderNode, OutputItem, Table};
use stat_core::dist::normal_cdf;
use stat_core::ols::{OlsError, OlsFit, candidate_stats, ols};

/// Casewise variables to save (SPSS /SAVE PRED RESID ZPRED ZRESID SRESID).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SaveFlags {
    pub predicted: bool,
    pub residual: bool,
    pub z_predicted: bool,
    pub z_residual: bool,
    pub studentized: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RegressionOptions {
    pub dependent: VarId,
    pub independents: Vec<VarId>,
    pub collinearity: bool,
    pub durbin_watson: bool,
    pub plot_hist: bool,
    pub plot_pp: bool,
    pub plot_zpred_sresid: bool,
    pub save: SaveFlags,
}

fn model_rows(children: Vec<String>) -> Vec<HeaderNode> {
    vec![HeaderNode { label: "1".into(), children: children.into_iter().map(|c| leaf(&c)).collect() }]
}

const F3: CellFormat = CellFormat::Fixed(3);
const F4: CellFormat = CellFormat::Fixed(4);

fn mean_sd(v: &[f64]) -> (f64, f64) {
    let n = v.len() as f64;
    let m = v.iter().sum::<f64>() / n;
    (m, (v.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / (n - 1.0)).sqrt())
}

/// Run the regression; tables and charts in SPSS order. Saved variables are appended to `ds`.
pub fn run_regression(ds: &mut Dataset, opt: &RegressionOptions) -> Vec<OutputItem> {
    let mut ids = vec![opt.dependent];
    ids.extend(&opt.independents);
    if opt.independents.is_empty() {
        return warning("Regression: no independent variables were specified.");
    }
    if let Some(&id) = ids.iter().find(|&&id| !matches!(ds.var(id).var_type, VarType::Numeric)) {
        return warning(format!("Regression: variable {} is not numeric.", ds.var(id).name));
    }
    let rows = ds.listwise_rows(&ids);
    let y: Vec<f64> = rows.iter().map(|&r| ds.numeric(opt.dependent)[r]).collect();
    let xcols: Vec<Vec<f64>> =
        opt.independents.iter().map(|&id| rows.iter().map(|&r| ds.numeric(id)[r]).collect()).collect();
    let xs: Vec<&[f64]> = xcols.iter().map(Vec::as_slice).collect();

    let fit = match ols(&y, &xs) {
        Ok(f) => f,
        Err(OlsError::TooFewCases) => {
            return warning(format!(
                "Regression: there are too few valid cases ({}) for the number of independent variables ({}). \
                 No analysis was performed.",
                rows.len(),
                opt.independents.len()
            ));
        }
        Err(OlsError::NoPredictors) => {
            return warning("Regression: no independent variable passed the tolerance test. No analysis was performed.");
        }
        Err(OlsError::ZeroVarianceDependent) => {
            return warning(format!(
                "Regression: the dependent variable {} has zero variance. No statistics are computed.",
                ds.var(opt.dependent).name
            ));
        }
        Err(e) => return warning(format!("Regression: {e}.")),
    };

    let dep_label = label(ds, opt.dependent);
    let dep_note = format!("Dependent Variable: {dep_label}");
    let kept: Vec<usize> = (0..opt.independents.len()).filter(|j| !fit.excluded.contains(j)).collect();
    let kept_labels: Vec<String> = kept.iter().map(|&j| label(ds, opt.independents[j])).collect();
    // SPSS lists entered variables last-entered first.
    let rev: Vec<String> = kept_labels.iter().rev().cloned().collect();
    let in_model = format!("(Constant), {}", rev.join(", "));
    let pred_note = format!("Predictors: {in_model}");

    let mut out = vec![OutputItem::Title("Regression".into())];
    out.push(OutputItem::Table(entered_table(&rev, &dep_note, !fit.excluded.is_empty())));
    out.push(OutputItem::Table(summary_table(&fit, opt, &pred_note, &dep_note)));
    out.push(OutputItem::Table(anova_table(&fit, &pred_note, &dep_note)));
    out.push(OutputItem::Table(coef_table(&fit, opt, &kept_labels, &dep_note)));
    if !fit.excluded.is_empty() {
        out.push(OutputItem::Table(excluded_table(ds, &fit, opt, &y, &xs, &kept, &in_model, &dep_note)));
    }
    if opt.collinearity {
        out.push(OutputItem::Table(collin_table(&fit, &kept_labels, &dep_note)));
    }
    let any_plot = opt.plot_hist || opt.plot_pp || opt.plot_zpred_sresid;
    let any_save = opt.save != SaveFlags::default();
    if any_plot || any_save {
        out.push(OutputItem::Table(residual_stats_table(&fit, &dep_note)));
    }
    let sub = format!("\n{dep_note}");
    if fit.z_residuals.iter().any(|v| !v.is_finite()) {
        out.push(OutputItem::Warning(
            "Regression: the fit is perfect (residual sum of squares is zero). Statistics that depend on \
             the residual variance are not computed and residual plots are not produced."
                .into(),
        ));
    }
    let zres: Vec<f64> = fit.z_residuals.iter().copied().filter(|v| v.is_finite()).collect();
    let points: Vec<(f64, f64)> = fit
        .z_predicted
        .iter()
        .copied()
        .zip(fit.studentized_residuals.iter().copied())
        .filter(|(a, b)| a.is_finite() && b.is_finite())
        .collect();
    if opt.plot_hist && !zres.is_empty() {
        out.push(OutputItem::Chart(Chart::Histogram {
            title: format!("Histogram{sub}"),
            values: zres.clone(),
            normal_curve: true,
        }));
    }
    if opt.plot_pp && !zres.is_empty() {
        let mut z = zres;
        z.sort_by(f64::total_cmp);
        let n = z.len() as f64;
        out.push(OutputItem::Chart(Chart::PPPlot {
            title: format!("Normal P-P Plot of Regression Standardized Residual{sub}"),
            observed_cum: (1..=z.len()).map(|i| i as f64 / (n + 1.0)).collect(),
            expected_cum: z.iter().map(|&v| normal_cdf(v)).collect(),
        }));
    }
    if opt.plot_zpred_sresid && !points.is_empty() {
        out.push(OutputItem::Chart(Chart::Scatter {
            title: format!("Scatterplot{sub}"),
            x_label: "Regression Standardized Predicted Value".into(),
            y_label: "Regression Studentized Residual".into(),
            points,
        }));
    }
    if let Err(msg) = save_vars(ds, &fit, &rows, opt.save) {
        out.push(OutputItem::Warning(msg));
    }
    out
}

fn entered_table(rev: &[String], dep_note: &str, any_excluded: bool) -> Table {
    let note_b = if any_excluded { "Tolerance = .000 limit reached." } else { "All requested variables entered." };
    Table {
        title: "Variables Entered/Removed".into(),
        col_headers: vec![leaf("Variables Entered"), leaf("Variables Removed"), leaf("Method")],
        row_headers: vec![leaf("1")],
        cells: vec![vec![foot(text(&rev.join(", ")), 'b'), text("."), text("Enter")]],
        footnotes: vec![('a', dep_note.into()), ('b', note_b.into())],
    }
}

fn summary_table(fit: &OlsFit, opt: &RegressionOptions, pred_note: &str, dep_note: &str) -> Table {
    let mut cols = vec![leaf("R"), leaf("R Square"), leaf("Adjusted R Square"), leaf("Std. Error of the Estimate")];
    let mut row = vec![foot(num(fit.r, F3), 'a'), num(fit.r2, F3), num(fit.adj_r2, F3), num(fit.se_est, CellFormat::Fixed(5))];
    if opt.durbin_watson {
        cols.push(leaf("Durbin-Watson"));
        row.push(num(fit.durbin_watson, F3));
    }
    Table {
        title: "Model Summary".into(),
        col_headers: cols,
        row_headers: vec![leaf("1")],
        cells: vec![row],
        footnotes: vec![('a', pred_note.into()), ('b', dep_note.into())],
    }
}

fn anova_table(fit: &OlsFit, pred_note: &str, dep_note: &str) -> Table {
    let ms_reg = fit.ss_reg / fit.df_reg as f64;
    let ms_res = fit.ss_res / fit.df_res as f64;
    let count = |v: usize| num(v as f64, CellFormat::Count);
    Table {
        title: "ANOVA".into(),
        col_headers: ["Sum of Squares", "df", "Mean Square", "F", "Sig."].map(leaf).to_vec(),
        row_headers: model_rows(vec!["Regression".into(), "Residual".into(), "Total".into()]),
        cells: vec![
            vec![num(fit.ss_reg, F3), count(fit.df_reg), num(ms_reg, F3), num(fit.f, F3), foot(num(fit.f_sig, CellFormat::Sig), 'b')],
            vec![num(fit.ss_res, F3), count(fit.df_res), num(ms_res, F3), empty(), empty()],
            vec![num(fit.ss_reg + fit.ss_res, F3), count(fit.df_reg + fit.df_res), empty(), empty(), empty()],
        ],
        footnotes: vec![('a', dep_note.into()), ('b', pred_note.into())],
    }
}

fn coef_table(fit: &OlsFit, opt: &RegressionOptions, kept_labels: &[String], dep_note: &str) -> Table {
    let mut cols = vec![
        group("Unstandardized Coefficients", &["B", "Std. Error"]),
        group("Standardized Coefficients", &["Beta"]),
        leaf("t"),
        leaf("Sig."),
    ];
    if opt.collinearity {
        cols.push(group("Collinearity Statistics", &["Tolerance", "VIF"]));
    }
    let mut names = vec!["(Constant)".to_string()];
    names.extend(kept_labels.iter().cloned());
    let cells = (0..fit.b.len())
        .map(|j| {
            let mut row = vec![
                num(fit.b[j], F3),
                num(fit.se[j], F3),
                num_or_empty(fit.beta[j], F3),
                num(fit.t[j], F3),
                num(fit.sig[j], CellFormat::Sig),
            ];
            if opt.collinearity {
                row.push(num_or_empty(fit.tolerance[j], F3));
                row.push(num_or_empty(fit.vif[j], F3));
            }
            row
        })
        .collect();
    Table {
        title: "Coefficients".into(),
        col_headers: cols,
        row_headers: model_rows(names),
        cells,
        footnotes: vec![('a', dep_note.into())],
    }
}

#[allow(clippy::too_many_arguments)]
fn excluded_table(
    ds: &Dataset,
    fit: &OlsFit,
    opt: &RegressionOptions,
    y: &[f64],
    xs: &[&[f64]],
    kept: &[usize],
    in_model: &str,
    dep_note: &str,
) -> Table {
    let entered: Vec<&[f64]> = kept.iter().map(|&j| xs[j]).collect();
    let mut stat_cols = vec!["Tolerance"];
    if opt.collinearity {
        stat_cols.extend(["VIF", "Minimum Tolerance"]);
    }
    let cells = fit
        .excluded
        .iter()
        .map(|&j| {
            let s = candidate_stats(y, &entered, xs[j]);
            let mut row = vec![
                foot(num(s.beta_in, F3), 'b'),
                num(s.t, F3),
                num(s.sig, CellFormat::Sig),
                num(s.partial, F3),
                num(s.tolerance, F3),
            ];
            if opt.collinearity {
                row.push(num(s.vif, F3));
                row.push(num(s.min_tolerance, F3));
            }
            row
        })
        .collect();
    Table {
        title: "Excluded Variables".into(),
        col_headers: vec![
            leaf("Beta In"),
            leaf("t"),
            leaf("Sig."),
            leaf("Partial Correlation"),
            group("Collinearity Statistics", &stat_cols),
        ],
        row_headers: model_rows(fit.excluded.iter().map(|&j| label(ds, opt.independents[j])).collect()),
        cells,
        footnotes: vec![('a', dep_note.into()), ('b', format!("Predictors in the Model: {in_model}"))],
    }
}

fn collin_table(fit: &OlsFit, kept_labels: &[String], dep_note: &str) -> Table {
    let mut names = vec!["(Constant)"];
    names.extend(kept_labels.iter().map(String::as_str));
    let cells = (0..fit.eigenvalues.len())
        .map(|k| {
            let mut row = vec![num(fit.eigenvalues[k], F3), num(fit.condition_index[k], F3)];
            row.extend(fit.variance_proportions[k].iter().map(|&p| num(p, CellFormat::Fixed(2))));
            row
        })
        .collect();
    Table {
        title: "Collinearity Diagnostics".into(),
        col_headers: vec![leaf("Eigenvalue"), leaf("Condition Index"), group("Variance Proportions", &names)],
        row_headers: model_rows((1..=fit.eigenvalues.len()).map(|d| d.to_string()).collect()),
        cells,
        footnotes: vec![('a', dep_note.into())],
    }
}

fn residual_stats_table(fit: &OlsFit, dep_note: &str) -> Table {
    let n = fit.predicted.len();
    let rows: [(&str, &[f64]); 4] = [
        ("Predicted Value", &fit.predicted),
        ("Residual", &fit.residuals),
        ("Std. Predicted Value", &fit.z_predicted),
        ("Std. Residual", &fit.z_residuals),
    ];
    let cells = rows
        .iter()
        .map(|(_, v)| {
            let min = v.iter().copied().fold(f64::INFINITY, f64::min);
            let max = v.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            let (m, sd) = mean_sd(v);
            vec![num(min, F4), num(max, F4), num(m, F4), num(sd, F4), num(n as f64, CellFormat::Count)]
        })
        .collect();
    Table {
        title: "Residuals Statistics".into(),
        col_headers: ["Minimum", "Maximum", "Mean", "Std. Deviation", "N"].map(leaf).to_vec(),
        row_headers: rows.iter().map(|(l, _)| leaf(l)).collect(),
        cells,
        footnotes: vec![('a', dep_note.into())],
    }
}

fn save_vars(ds: &mut Dataset, fit: &OlsFit, rows: &[usize], save: SaveFlags) -> Result<(), String> {
    let specs: [(bool, &str, &str, &[f64]); 5] = [
        (save.predicted, "PRE_", "Unstandardized Predicted Value", &fit.predicted),
        (save.residual, "RES_", "Unstandardized Residual", &fit.residuals),
        (save.z_predicted, "ZPR_", "Standardized Predicted Value", &fit.z_predicted),
        (save.z_residual, "ZRE_", "Standardized Residual", &fit.z_residuals),
        (save.studentized, "SRE_", "Studentized Residual", &fit.studentized_residuals),
    ];
    for (on, prefix, lab, vals) in specs {
        if !on {
            continue;
        }
        let mut col = vec![f64::NAN; ds.n_cases()];
        for (&r, &v) in rows.iter().zip(vals) {
            col[r] = v;
        }
        let mut var = Variable::numeric(&ds.unique_name(prefix));
        var.label = Some(lab.into());
        var.width = 11;
        var.decimals = 5;
        ds.add_variable(var, Column::Numeric(col)).map_err(|e| format!("Regression: could not save {prefix}: {e}"))?;
    }
    Ok(())
}
