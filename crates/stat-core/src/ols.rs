//! Ordinary least squares with an intercept (SPSS REGRESSION, method Enter).
//!
//! The fit uses Householder QR on mean-centred predictors (numerically robust for
//! ill-conditioned data such as Longley) plus one step of iterative refinement.
use crate::dist::{f_sig, t_sig_two_tailed};
use nalgebra::{DMatrix, DVector, SymmetricEigen};

/// Predictors whose tolerance against the already-entered predictors falls below this
/// value are excluded (SPSS default TOLERANCE criterion).
pub const TOLERANCE_LIMIT: f64 = 0.0001;

#[derive(Debug, Clone, PartialEq)]
pub struct OlsFit {
    /// Coefficients: index 0 = constant, then retained predictors in entry order.
    pub b: Vec<f64>,
    pub se: Vec<f64>,
    /// Standardized coefficients (NaN for the constant).
    pub beta: Vec<f64>,
    pub t: Vec<f64>,
    pub sig: Vec<f64>,
    pub r: f64,
    pub r2: f64,
    pub adj_r2: f64,
    pub se_est: f64,
    pub ss_reg: f64,
    pub ss_res: f64,
    pub df_reg: usize,
    pub df_res: usize,
    pub f: f64,
    pub f_sig: f64,
    pub durbin_watson: f64,
    /// Same indexing as `b`; NaN for the constant.
    pub tolerance: Vec<f64>,
    pub vif: Vec<f64>,
    /// Eigenvalues of the unit-length-scaled X'X (constant included), descending.
    pub eigenvalues: Vec<f64>,
    pub condition_index: Vec<f64>,
    /// `[dimension][coefficient]`, coefficient indexing as in `b`.
    pub variance_proportions: Vec<Vec<f64>>,
    pub predicted: Vec<f64>,
    pub residuals: Vec<f64>,
    pub z_predicted: Vec<f64>,
    pub z_residuals: Vec<f64>,
    pub studentized_residuals: Vec<f64>,
    /// Indices into `xs` of predictors removed because tolerance < 0.0001.
    pub excluded: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum OlsError {
    /// Fewer than (number of predictors + 2) cases: no residual degrees of freedom.
    TooFewCases,
    /// A predictor's length differs from `y`.
    LengthMismatch,
    /// Every predictor was excluded (or none was given).
    NoPredictors,
}

impl std::fmt::Display for OlsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            OlsError::TooFewCases => "too few cases for the number of predictors",
            OlsError::LengthMismatch => "predictor length differs from dependent length",
            OlsError::NoPredictors => "no predictors could be entered",
        })
    }
}

impl std::error::Error for OlsError {}

fn mean(v: &[f64]) -> f64 {
    v.iter().sum::<f64>() / v.len() as f64
}

fn centered(v: &[f64]) -> Vec<f64> {
    let m = mean(v);
    v.iter().map(|x| x - m).collect()
}

fn sd(v: &[f64]) -> f64 {
    let m = mean(v);
    (v.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / (v.len() as f64 - 1.0)).sqrt()
}

/// Least-squares solve of `a * b = y` via QR with one refinement step.
/// Returns (coefficients, thin Q, R).
fn ls_solve(a: &DMatrix<f64>, y: &DVector<f64>) -> Option<(DVector<f64>, DMatrix<f64>, DMatrix<f64>)> {
    let qr = a.clone().qr();
    let q = qr.q();
    let r = qr.r();
    let solve = |rhs: &DVector<f64>| r.solve_upper_triangular(&(q.transpose() * rhs));
    let mut b = solve(y)?;
    let resid = y - a * &b;
    if let Some(db) = solve(&resid) {
        b += db;
    }
    Some((b, q, r))
}

/// Tolerance of `x` (centred) against centred columns `others`: 1 - R².
fn tolerance_of(x: &[f64], others: &[Vec<f64>]) -> f64 {
    let n = x.len();
    let sst: f64 = x.iter().map(|v| v * v).sum();
    if sst <= 0.0 || !sst.is_finite() {
        return 0.0;
    }
    if others.is_empty() {
        return 1.0;
    }
    let a = DMatrix::from_fn(n, others.len(), |i, j| others[j][i]);
    let yv = DVector::from_column_slice(x);
    match ls_solve(&a, &yv) {
        Some((b, _, _)) => {
            let sse = (&yv - &a * b).norm_squared();
            (sse / sst).clamp(0.0, 1.0)
        }
        None => 0.0,
    }
}

/// Fit `y` on `xs` with an intercept.
pub fn ols(y: &[f64], xs: &[&[f64]]) -> Result<OlsFit, OlsError> {
    let n = y.len();
    if xs.iter().any(|x| x.len() != n) {
        return Err(OlsError::LengthMismatch);
    }
    if xs.is_empty() {
        return Err(OlsError::NoPredictors);
    }
    if n < xs.len() + 2 {
        return Err(OlsError::TooFewCases);
    }

    // Entry-order tolerance screening.
    let all_c: Vec<Vec<f64>> = xs.iter().map(|x| centered(x)).collect();
    let mut kept: Vec<usize> = vec![];
    let mut excluded = vec![];
    for j in 0..xs.len() {
        let others: Vec<Vec<f64>> = kept.iter().map(|&k| all_c[k].clone()).collect();
        if tolerance_of(&all_c[j], &others) < TOLERANCE_LIMIT {
            excluded.push(j);
        } else {
            kept.push(j);
        }
    }
    if kept.is_empty() {
        return Err(OlsError::NoPredictors);
    }
    let k = kept.len();
    let nf = n as f64;
    let ybar = mean(y);
    let xbar: Vec<f64> = kept.iter().map(|&j| mean(xs[j])).collect();

    // Fit on centred data.
    let xc = DMatrix::from_fn(n, k, |i, j| all_c[kept[j]][i]);
    let yc = DVector::from_iterator(n, y.iter().map(|v| v - ybar));
    let (slopes, q, r) = ls_solve(&xc, &yc).ok_or(OlsError::NoPredictors)?;
    let b0 = ybar - slopes.iter().zip(&xbar).map(|(b, m)| b * m).sum::<f64>();
    let mut b = vec![b0];
    b.extend(slopes.iter());

    let predicted: Vec<f64> = (0..n)
        .map(|i| b0 + kept.iter().enumerate().map(|(jj, &j)| slopes[jj] * xs[j][i]).sum::<f64>())
        .collect();
    let residuals: Vec<f64> = y.iter().zip(&predicted).map(|(a, p)| a - p).collect();
    let ss_res: f64 = residuals.iter().map(|e| e * e).sum();
    let sst: f64 = yc.norm_squared();
    let ss_reg: f64 = predicted.iter().map(|p| (p - ybar) * (p - ybar)).sum();
    let df_reg = k;
    let df_res = n - k - 1;
    let ms_res = ss_res / df_res as f64;
    let se_est = ms_res.sqrt();
    let r2 = if sst > 0.0 { 1.0 - ss_res / sst } else { f64::NAN };
    let adj_r2 = 1.0 - (1.0 - r2) * (nf - 1.0) / df_res as f64;
    let f = (ss_reg / df_reg as f64) / ms_res;

    // Covariance of slopes: s² (Xc'Xc)^-1 = s² R^-1 R^-T.
    let rinv = r.clone().try_inverse().unwrap_or_else(|| DMatrix::from_element(k, k, f64::NAN));
    let cinv = &rinv * rinv.transpose();
    let xb = DVector::from_column_slice(&xbar);
    let var_b0 = ms_res * (1.0 / nf + (xb.transpose() * &cinv * &xb)[(0, 0)]);
    let mut se = vec![var_b0.sqrt()];
    se.extend((0..k).map(|j| (ms_res * cinv[(j, j)]).sqrt()));
    let t: Vec<f64> = b.iter().zip(&se).map(|(b, s)| b / s).collect();
    let sig: Vec<f64> = t.iter().map(|&t| t_sig_two_tailed(t, df_res as f64)).collect();

    let sd_y = sd(y);
    let mut beta = vec![f64::NAN];
    beta.extend(kept.iter().enumerate().map(|(jj, &j)| slopes[jj] * sd(xs[j]) / sd_y));

    let mut tolerance = vec![f64::NAN];
    for (jj, &j) in kept.iter().enumerate() {
        let others: Vec<Vec<f64>> =
            kept.iter().enumerate().filter(|&(ii, _)| ii != jj).map(|(_, &o)| all_c[o].clone()).collect();
        tolerance.push(tolerance_of(&all_c[j], &others));
    }
    let vif: Vec<f64> = tolerance.iter().map(|t| 1.0 / t).collect();

    // Durbin-Watson.
    let dw_num: f64 = residuals.windows(2).map(|w| (w[1] - w[0]) * (w[1] - w[0])).sum();
    let durbin_watson = dw_num / ss_res;

    // Standardized / studentized values.
    let (mp, sp) = (mean(&predicted), sd(&predicted));
    let z_predicted: Vec<f64> = predicted.iter().map(|p| (p - mp) / sp).collect();
    let z_residuals: Vec<f64> = residuals.iter().map(|e| e / se_est).collect();
    let studentized_residuals: Vec<f64> = (0..n)
        .map(|i| {
            let h = 1.0 / nf + q.row(i).norm_squared();
            residuals[i] / (se_est * (1.0 - h).sqrt())
        })
        .collect();

    // Collinearity diagnostics on the unit-length-scaled design matrix (constant included).
    let p = k + 1;
    let x = DMatrix::from_fn(n, p, |i, j| if j == 0 { 1.0 } else { xs[kept[j - 1]][i] });
    let mut xs_scaled = x;
    for mut c in xs_scaled.column_iter_mut() {
        let norm = c.norm();
        if norm > 0.0 {
            c /= norm;
        }
    }
    let eig = SymmetricEigen::new(xs_scaled.transpose() * &xs_scaled);
    let mut order: Vec<usize> = (0..p).collect();
    order.sort_by(|&a, &b| eig.eigenvalues[b].total_cmp(&eig.eigenvalues[a]));
    let eigenvalues: Vec<f64> = order.iter().map(|&d| eig.eigenvalues[d]).collect();
    let condition_index: Vec<f64> = eigenvalues.iter().map(|l| (eigenvalues[0] / l).sqrt()).collect();
    let phi: Vec<Vec<f64>> = order
        .iter()
        .map(|&d| (0..p).map(|j| eig.eigenvectors[(j, d)].powi(2) / eig.eigenvalues[d]).collect())
        .collect();
    let totals: Vec<f64> = (0..p).map(|j| phi.iter().map(|row| row[j]).sum()).collect();
    let variance_proportions: Vec<Vec<f64>> =
        phi.iter().map(|row| row.iter().zip(&totals).map(|(v, t)| v / t).collect()).collect();

    Ok(OlsFit {
        b,
        se,
        beta,
        t,
        sig,
        r: r2.sqrt(),
        r2,
        adj_r2,
        se_est,
        ss_reg,
        ss_res,
        df_reg,
        df_res,
        f,
        f_sig: f_sig(f, df_reg as f64, df_res as f64),
        durbin_watson,
        tolerance,
        vif,
        eigenvalues,
        condition_index,
        variance_proportions,
        predicted,
        residuals,
        z_predicted,
        z_residuals,
        studentized_residuals,
        excluded,
    })
}

/// Statistics for a predictor not in the equation ("Excluded Variables" table).
#[derive(Debug, Clone, PartialEq)]
pub struct CandidateStats {
    /// Standardized coefficient the candidate would get if entered.
    pub beta_in: f64,
    pub t: f64,
    pub sig: f64,
    /// Partial correlation with the dependent, controlling for the entered predictors.
    pub partial: f64,
    /// Tolerance of the candidate against the entered predictors.
    pub tolerance: f64,
    pub vif: f64,
    /// Smallest tolerance among all predictors of the equation if the candidate entered.
    pub min_tolerance: f64,
}

/// Below this tolerance the candidate is treated as an exact linear combination of the
/// entered predictors and its coefficient statistics are undefined (NaN).
const SINGULAR_TOLERANCE: f64 = 1e-10;

/// Statistics for `candidate` given that `entered` (with intercept) are in the equation.
pub fn candidate_stats(y: &[f64], entered: &[&[f64]], candidate: &[f64]) -> CandidateStats {
    let n = y.len();
    let ent_c: Vec<Vec<f64>> = entered.iter().map(|x| centered(x)).collect();
    let cand_c = centered(candidate);
    let tolerance = tolerance_of(&cand_c, &ent_c);

    let mut all = ent_c.clone();
    all.push(cand_c.clone());
    let min_tolerance = (0..all.len())
        .map(|j| {
            let others: Vec<Vec<f64>> =
                all.iter().enumerate().filter(|&(i, _)| i != j).map(|(_, v)| v.clone()).collect();
            tolerance_of(&all[j], &others)
        })
        .fold(f64::INFINITY, f64::min);

    let undefined = CandidateStats {
        beta_in: f64::NAN,
        t: f64::NAN,
        sig: f64::NAN,
        partial: f64::NAN,
        tolerance,
        vif: f64::NAN,
        min_tolerance,
    };
    let df = n as f64 - entered.len() as f64 - 2.0;
    if tolerance < SINGULAR_TOLERANCE || df < 1.0 {
        return undefined;
    }
    let resid = |v: &[f64]| -> DVector<f64> {
        let yv = DVector::from_column_slice(v);
        if ent_c.is_empty() {
            return yv;
        }
        let a = DMatrix::from_fn(n, ent_c.len(), |i, j| ent_c[j][i]);
        match ls_solve(&a, &yv) {
            Some((b, _, _)) => &yv - &a * b,
            None => DVector::from_element(n, f64::NAN),
        }
    };
    let ey = resid(&centered(y));
    let ex = resid(&cand_c);
    let sxy = ey.dot(&ex);
    let (sxx, syy) = (ex.norm_squared(), ey.norm_squared());
    let partial = sxy / (sxx * syy).sqrt();
    let b = sxy / sxx;
    let t = partial * (df / (1.0 - partial * partial)).sqrt();
    CandidateStats {
        beta_in: b * sd(candidate) / sd(y),
        t,
        sig: t_sig_two_tailed(t, df),
        partial,
        tolerance,
        vif: 1.0 / tolerance,
        min_tolerance,
    }
}
