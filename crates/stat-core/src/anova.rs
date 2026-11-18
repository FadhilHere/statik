//! One-way analysis of variance.
use crate::dist::f_sig;

#[derive(Debug, Clone, PartialEq)]
pub struct OneWay {
    pub ss_between: f64,
    pub ss_within: f64,
    pub df_between: usize,
    pub df_within: usize,
    /// NaN when the within-groups sum of squares is 0 (F undefined).
    pub f: f64,
    pub sig: f64,
}

/// One-way ANOVA; `None` with fewer than 2 non-empty groups or no within-groups degrees of freedom.
/// Empty groups are not allowed (the caller drops them).
pub fn oneway(groups: &[&[f64]]) -> Option<OneWay> {
    let k = groups.len();
    if k < 2 || groups.iter().any(|g| g.is_empty()) {
        return None;
    }
    let n: usize = groups.iter().map(|g| g.len()).sum();
    if n <= k {
        return None;
    }
    let grand = groups.iter().flat_map(|g| g.iter()).sum::<f64>() / n as f64;
    let (mut ssb, mut ssw) = (0.0, 0.0);
    for g in groups {
        let m = g.iter().sum::<f64>() / g.len() as f64;
        ssb += g.len() as f64 * (m - grand) * (m - grand);
        // Constant group: rounding in the mean must not leave a tiny non-zero spread.
        let constant = g.iter().all(|&v| v == g[0]);
        if !constant {
            ssw += g.iter().map(|v| (v - m) * (v - m)).sum::<f64>();
        }
    }
    let (df_between, df_within) = (k - 1, n - k);
    let (f, sig) = if ssw == 0.0 {
        (f64::NAN, f64::NAN)
    } else {
        let f = (ssb / df_between as f64) / (ssw / df_within as f64);
        (f, f_sig(f, df_between as f64, df_within as f64))
    };
    Some(OneWay { ss_between: ssb, ss_within: ssw, df_between, df_within, f, sig })
}
