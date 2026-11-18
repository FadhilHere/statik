//! Descriptive statistics following SPSS definitions.

#[derive(Debug, Clone, PartialEq)]
pub struct Describe {
    pub n: usize,
    pub sum: f64,
    pub mean: f64,
    pub variance: f64,
    pub sd: f64,
    pub se_mean: f64,
    pub min: f64,
    pub max: f64,
    pub range: f64,
    pub skewness: Option<f64>,
    pub se_skewness: Option<f64>,
    pub kurtosis: Option<f64>,
    pub se_kurtosis: Option<f64>,
}

/// Descriptives of `x`; `None` when empty. Variance uses n-1 (NaN for n == 1).
pub fn describe(x: &[f64]) -> Option<Describe> {
    if x.is_empty() {
        return None;
    }
    let n = x.len();
    let nf = n as f64;
    let sum: f64 = x.iter().sum();
    let mean = sum / nf;
    let (mut m2, mut m3, mut m4) = (0.0, 0.0, 0.0);
    let (mut min, mut max) = (f64::INFINITY, f64::NEG_INFINITY);
    for &v in x {
        let d = v - mean;
        let d2 = d * d;
        m2 += d2;
        m3 += d2 * d;
        m4 += d2 * d2;
        min = min.min(v);
        max = max.max(v);
    }
    // Constant data: rounding in the mean would otherwise leave a tiny non-zero spread.
    let constant = min == max;
    let variance = if n < 2 {
        f64::NAN
    } else if constant {
        0.0
    } else {
        m2 / (nf - 1.0)
    };
    let sd = variance.sqrt();
    let se_mean = sd / nf.sqrt();

    let (mut skewness, mut se_skewness) = (None, None);
    let (mut kurtosis, mut se_kurtosis) = (None, None);
    if n >= 3 && sd > 0.0 && !constant {
        // sum of z^3 with z = d / sd
        skewness = Some(nf / ((nf - 1.0) * (nf - 2.0)) * m3 / (sd * sd * sd));
    }
    if n >= 3 {
        se_skewness = Some((6.0 * nf * (nf - 1.0) / ((nf - 2.0) * (nf + 1.0) * (nf + 3.0))).sqrt());
    }
    if n >= 4 && sd > 0.0 && !constant {
        let s4 = m4 / (sd * sd * sd * sd);
        kurtosis = Some(
            nf * (nf + 1.0) / ((nf - 1.0) * (nf - 2.0) * (nf - 3.0)) * s4
                - 3.0 * (nf - 1.0) * (nf - 1.0) / ((nf - 2.0) * (nf - 3.0)),
        );
    }
    if let (4.., Some(se_s)) = (n, se_skewness) {
        se_kurtosis = Some(2.0 * se_s * ((nf * nf - 1.0) / ((nf - 3.0) * (nf + 5.0))).sqrt());
    }
    Some(Describe {
        n, sum, mean, variance, sd, se_mean, min, max, range: max - min,
        skewness, se_skewness, kurtosis, se_kurtosis,
    })
}

/// SPSS HAVERAGE percentile on an ascending-sorted slice; `p` in 0..=100.
pub fn percentile_haverage(sorted: &[f64], p: f64) -> f64 {
    let n = sorted.len();
    assert!(n > 0, "empty slice");
    let pos = (n as f64 + 1.0) * p / 100.0;
    if pos < 1.0 {
        return sorted[0];
    }
    if pos >= n as f64 {
        return sorted[n - 1];
    }
    let lo = pos.floor();
    let frac = pos - lo;
    let i = lo as usize; // 1-based rank
    sorted[i - 1] + frac * (sorted[i] - sorted[i - 1])
}

/// SPSS EXAMINE 5% trimmed mean on an ascending-sorted slice (fractional trimming).
pub fn trimmed_mean_5(sorted: &[f64]) -> f64 {
    let n = sorted.len();
    assert!(n > 0, "empty slice");
    let t = 0.05 * n as f64;
    let (mut num, mut den) = (0.0, 0.0);
    for (i, &v) in sorted.iter().enumerate() {
        // mass trimmed from the low end / high end of case i
        let low = (t - i as f64).clamp(0.0, 1.0);
        let high = (t - (n - 1 - i) as f64).clamp(0.0, 1.0);
        let w = (1.0 - low - high).max(0.0);
        num += w * v;
        den += w;
    }
    num / den
}

/// True when `values`, computed from inputs whose largest magnitude is `scale`, are constant up
/// to floating-point rounding: max - min <= 8·ε·scale. Empty input counts as constant.
pub fn is_constant_computed(values: &[f64], scale: f64) -> bool {
    let (lo, hi) = values.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(l, h), &v| (l.min(v), h.max(v)));
    values.is_empty() || hi - lo <= 8.0 * f64::EPSILON * scale.abs()
}
