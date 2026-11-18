//! Runs test about the median (SPSS NPAR TESTS /RUNS(MEDIAN)).
use crate::describe::percentile_haverage;
use crate::dist::normal_cdf;

#[derive(Debug, Clone, PartialEq)]
pub struct RunsResult {
    pub test_value: f64,
    pub n_below: usize,
    pub n_at_or_above: usize,
    pub runs: usize,
    pub z: f64,
    pub sig: f64,
}

/// Runs test with cut point = median (HAVERAGE). Cases < median form one group, cases >= median the other.
/// `None` when either group is empty (e.g. constant data) or n < 2.
pub fn runs_test_median(x: &[f64]) -> Option<RunsResult> {
    let n = x.len();
    if n < 2 {
        return None;
    }
    let mut sorted = x.to_vec();
    sorted.sort_by(f64::total_cmp);
    let median = percentile_haverage(&sorted, 50.0);
    let below: Vec<bool> = x.iter().map(|&v| v < median).collect();
    let n1 = below.iter().filter(|&&b| b).count();
    let n2 = n - n1;
    if n1 == 0 || n2 == 0 {
        return None;
    }
    let runs = 1 + below.windows(2).filter(|w| w[0] != w[1]).count();
    let (nf, f1, f2) = (n as f64, n1 as f64, n2 as f64);
    let mu = 2.0 * f1 * f2 / nf + 1.0;
    let var = 2.0 * f1 * f2 * (2.0 * f1 * f2 - nf) / (nf * nf * (nf - 1.0));
    let mut d = runs as f64 - mu;
    if n < 50 {
        d = if d.abs() < 0.5 { 0.0 } else { d - 0.5 * d.signum() };
    }
    let z = d / var.sqrt();
    let sig = if z.is_finite() { (2.0 * normal_cdf(-z.abs())).min(1.0) } else { f64::NAN };
    Some(RunsResult { test_value: median, n_below: n1, n_at_or_above: n2, runs, z, sig })
}
