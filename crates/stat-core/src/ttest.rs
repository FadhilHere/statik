//! Student t tests (one-sample, independent, paired) and Levene's test (mean-centred), SPSS definitions.
use crate::describe::describe;
use crate::dist::{f_sig, t_quantile, t_sig_two_tailed};

#[derive(Debug, Clone, PartialEq)]
pub struct TResult {
    pub t: f64,
    pub df: f64,
    /// Two-sided significance.
    pub sig: f64,
    pub mean_diff: f64,
    pub se_diff: f64,
    pub ci: (f64, f64),
}

#[derive(Debug, Clone, PartialEq)]
pub struct IndResult {
    pub levene_f: f64,
    pub levene_sig: f64,
    pub equal: TResult,
    pub welch: TResult,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PairedResult {
    pub corr: f64,
    pub corr_sig: f64,
    pub test: TResult,
    pub sd_diff: f64,
}

fn make(mean_diff: f64, se: f64, df: f64, conf: f64) -> TResult {
    let t = mean_diff / se;
    let q = t_quantile(1.0 - (1.0 - conf) / 2.0, df);
    TResult { t, df, sig: t_sig_two_tailed(t, df), mean_diff, se_diff: se, ci: (mean_diff - q * se, mean_diff + q * se) }
}

/// One-sample t test against `test_value`. `None` when n < 2 or the sd is 0.
pub fn one_sample_t(x: &[f64], test_value: f64, conf: f64) -> Option<TResult> {
    let d = describe(x)?;
    if d.n < 2 || d.min == d.max || d.sd.is_nan() || !d.sd.is_finite() {
        return None;
    }
    Some(make(d.mean - test_value, d.se_mean, (d.n - 1) as f64, conf))
}

/// Levene's test centred on group means: (F, sig, df1, df2).
pub fn levene_mean(groups: &[&[f64]]) -> (f64, f64, usize, usize) {
    let k = groups.len();
    let z: Vec<Vec<f64>> = groups
        .iter()
        .map(|g| {
            // Constant group: rounding in the mean must not leave tiny non-zero deviations.
            if g.iter().all(|&v| v == g[0]) {
                return vec![0.0; g.len()];
            }
            let m = g.iter().sum::<f64>() / g.len() as f64;
            g.iter().map(|v| (v - m).abs()).collect()
        })
        .collect();
    let n: usize = z.iter().map(Vec::len).sum();
    let grand = z.iter().flatten().sum::<f64>() / n as f64;
    let (mut ssb, mut ssw) = (0.0, 0.0);
    for g in &z {
        let m = g.iter().sum::<f64>() / g.len() as f64;
        ssb += g.len() as f64 * (m - grand) * (m - grand);
        ssw += g.iter().map(|v| (v - m) * (v - m)).sum::<f64>();
    }
    let (df1, df2) = (k - 1, n - k);
    if ssw == 0.0 {
        return (f64::NAN, f64::NAN, df1, df2);
    }
    let f = (ssb / df1 as f64) / (ssw / df2 as f64);
    (f, f_sig(f, df1 as f64, df2 as f64), df1, df2)
}

/// Independent-samples t test (pooled and Welch) with Levene's test. `None` when a group has n < 2
/// or both groups have sd 0.
pub fn independent_t(a: &[f64], b: &[f64], conf: f64) -> Option<IndResult> {
    if a.len() < 2 || b.len() < 2 {
        return None;
    }
    let (da, db) = (describe(a)?, describe(b)?);
    if da.min == da.max && db.min == db.max {
        return None;
    }
    let (na, nb) = (da.n as f64, db.n as f64);
    let sp2 = ((na - 1.0) * da.variance + (nb - 1.0) * db.variance) / (na + nb - 2.0);
    if sp2.is_nan() || !sp2.is_finite() {
        return None;
    }
    let md = da.mean - db.mean;
    let equal = make(md, (sp2 * (1.0 / na + 1.0 / nb)).sqrt(), na + nb - 2.0, conf);
    let (va, vb) = (da.variance / na, db.variance / nb);
    let df_w = (va + vb) * (va + vb) / (va * va / (na - 1.0) + vb * vb / (nb - 1.0));
    let welch = make(md, (va + vb).sqrt(), df_w, conf);
    let (levene_f, levene_sig, _, _) = levene_mean(&[a, b]);
    Some(IndResult { levene_f, levene_sig, equal, welch })
}

/// Paired-samples t test on `a - b` plus the Pearson correlation. `None` when n < 2 or the
/// differences have sd 0.
pub fn paired_t(a: &[f64], b: &[f64], conf: f64) -> Option<PairedResult> {
    if a.len() != b.len() || a.len() < 2 {
        return None;
    }
    let diff: Vec<f64> = a.iter().zip(b).map(|(x, y)| x - y).collect();
    let test = one_sample_t(&diff, 0.0, conf)?;
    let (corr, corr_sig) = paired_corr(a, b);
    let sd_diff = test.se_diff * (a.len() as f64).sqrt();
    Some(PairedResult { corr, corr_sig, test, sd_diff })
}

/// Pearson correlation of two paired samples and its two-sided significance; NaN when either
/// variable is constant (or n < 2; the significance also needs n >= 3).
pub fn paired_corr(a: &[f64], b: &[f64]) -> (f64, f64) {
    let const_of = |x: &[f64]| x.iter().all(|&v| v == x[0]);
    if a.len() != b.len() || a.len() < 2 || const_of(a) || const_of(b) {
        return (f64::NAN, f64::NAN);
    }
    let n = a.len() as f64;
    let (ma, mb) = (a.iter().sum::<f64>() / n, b.iter().sum::<f64>() / n);
    let (mut sab, mut saa, mut sbb) = (0.0, 0.0, 0.0);
    for (x, y) in a.iter().zip(b) {
        sab += (x - ma) * (y - mb);
        saa += (x - ma) * (x - ma);
        sbb += (y - mb) * (y - mb);
    }
    let corr = (sab / (saa * sbb).sqrt()).clamp(-1.0, 1.0);
    let corr_sig = if a.len() < 3 {
        f64::NAN
    } else if corr.abs() >= 1.0 {
        0.0
    } else {
        t_sig_two_tailed(corr * ((n - 2.0) / (1.0 - corr * corr)).sqrt(), n - 2.0)
    };
    (corr, corr_sig)
}
