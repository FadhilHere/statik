//! Normality tests: Kolmogorov-Smirnov with Lilliefors correction and Shapiro-Wilk.
use crate::dist::{normal_cdf, normal_quantile};

/// Kolmogorov-Smirnov (Lilliefors) result.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KsResult {
    /// Most extreme absolute difference, max(d_pos, -d_neg).
    pub d: f64,
    /// Most extreme positive difference (≥ 0).
    pub d_pos: f64,
    /// Most extreme negative difference (≤ 0), signed as SPSS displays it.
    pub d_neg: f64,
    /// Lilliefors significance (Dallal-Wilkinson approximation), capped at 0.2.
    pub sig: f64,
    /// True when the approximation exceeded 0.2 and `sig` is the lower bound 0.2.
    pub sig_is_lower_bound: bool,
}

/// Shapiro-Wilk result.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SwResult {
    pub w: f64,
    pub sig: f64,
}

fn sorted(x: &[f64]) -> Vec<f64> {
    let mut v = x.to_vec();
    v.sort_by(f64::total_cmp);
    v
}

/// Dallal & Wilkinson (1986) approximation of the Lilliefors p-value
/// (as in statsmodels `pval_lf`); for n > 100, D is scaled by (n/100)^0.49 and n set to 100.
fn dallal_wilkinson(d: f64, n: usize) -> f64 {
    let (mut d, mut n) = (d, n as f64);
    if n > 100.0 {
        d *= (n / 100.0).powf(0.49);
        n = 100.0;
    }
    (-7.01256 * d * d * (n + 2.78019) + 2.99587 * d * (n + 2.78019).sqrt() - 0.122119 + 0.974598 / n.sqrt()
        + 1.67997 / n)
        .exp()
}

/// One-sample K-S test for normality with mean and sd (n − 1) estimated from the sample
/// (Lilliefors). `None` when n < 5, the data are constant (max == min; rounding in the mean can
/// leave a tiny non-zero sd) or the standard deviation is zero / not finite.
pub fn lilliefors(x: &[f64]) -> Option<KsResult> {
    let n = x.len();
    if n < 5 || x.iter().all(|&v| v == x[0]) {
        return None;
    }
    let nf = n as f64;
    let mean = x.iter().sum::<f64>() / nf;
    let sd = (x.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>() / (nf - 1.0)).sqrt();
    if !(sd.is_finite() && sd > 0.0) {
        return None;
    }
    let (mut d_pos, mut d_neg) = (0.0f64, 0.0f64);
    for (i, &v) in sorted(x).iter().enumerate() {
        let f = normal_cdf((v - mean) / sd);
        d_pos = d_pos.max((i + 1) as f64 / nf - f);
        d_neg = d_neg.max(f - i as f64 / nf);
    }
    let d = d_pos.max(d_neg);
    let p = dallal_wilkinson(d, n);
    let (sig, sig_is_lower_bound) = if p > 0.2 { (0.2, true) } else { (p, false) };
    Some(KsResult { d, d_pos, d_neg: -d_neg, sig, sig_is_lower_bound })
}

/// Evaluate c[0] + c[1]x + c[2]x² + … (AS R94 `poly`).
fn poly(c: &[f64], x: f64) -> f64 {
    c.iter().rev().fold(0.0, |acc, &ci| acc * x + ci)
}

/// Shapiro-Wilk W test, a port of Royston (1995) Algorithm AS R94 (`swilk`), including its
/// p-value approximations (exact for n = 3, 4 ≤ n ≤ 11, n ≥ 12).
/// `None` when n < 3, n > 5000 or the data have zero range.
pub fn shapiro_wilk(x: &[f64]) -> Option<SwResult> {
    const SMALL: f64 = 1e-19;
    const G: [f64; 2] = [-2.273, 0.459];
    const C1: [f64; 6] = [0.0, 0.221157, -0.147981, -2.07119, 4.434685, -2.706056];
    const C2: [f64; 6] = [0.0, 0.042981, -0.293762, -1.752461, 5.682633, -3.582633];
    const C3: [f64; 4] = [0.544, -0.39978, 0.025054, -6.714e-4];
    const C4: [f64; 4] = [1.3822, -0.77857, 0.062767, -0.0020322];
    const C5: [f64; 4] = [-1.5861, -0.31082, -0.083751, 0.0038915];
    const C6: [f64; 3] = [-0.4803, -0.082676, 0.0030302];

    let n = x.len();
    if !(3..=5000).contains(&n) || x.iter().any(|v| !v.is_finite()) {
        return None;
    }
    let x = sorted(x);
    let an = n as f64;
    let nn2 = n / 2;

    // Coefficients a[1..=nn2] (1-based as in AS R94; a[0] unused), for the upper half.
    let mut a = vec![0.0; nn2 + 1];
    if n == 3 {
        a[1] = 0.5f64.sqrt();
    } else {
        let an25 = an + 0.25;
        let m: Vec<f64> =
            (0..=nn2).map(|i| if i == 0 { 0.0 } else { normal_quantile((i as f64 - 0.375) / an25) }).collect();
        let summ2 = 2.0 * m[1..].iter().map(|v| v * v).sum::<f64>();
        let ssumm2 = summ2.sqrt();
        let rsn = 1.0 / an.sqrt();
        let a1 = poly(&C1, rsn) - m[1] / ssumm2;
        let (i1, fac) = if n > 5 {
            let a2 = -m[2] / ssumm2 + poly(&C2, rsn);
            a[2] = a2;
            (3, ((summ2 - 2.0 * m[1] * m[1] - 2.0 * m[2] * m[2]) / (1.0 - 2.0 * a1 * a1 - 2.0 * a2 * a2)).sqrt())
        } else {
            (2, ((summ2 - 2.0 * m[1] * m[1]) / (1.0 - 2.0 * a1 * a1)).sqrt())
        };
        a[1] = a1;
        for i in i1..=nn2 {
            a[i] = -m[i] / fac;
        }
    }

    let range = x[n - 1] - x[0];
    if range < SMALL {
        return None;
    }
    // Full antisymmetric coefficient vector for the ascending sample (i, j 1-based, j = n + 1 − i).
    let coef = |i: usize| -> f64 {
        let j = n + 1 - i;
        match i.cmp(&j) {
            std::cmp::Ordering::Less => -a[i],
            std::cmp::Ordering::Greater => a[j],
            std::cmp::Ordering::Equal => 0.0,
        }
    };
    let sa = (1..=n).map(coef).sum::<f64>() / an;
    let sx = x.iter().map(|v| v / range).sum::<f64>() / an;
    let (mut ssa, mut ssx, mut sax) = (0.0, 0.0, 0.0);
    for i in 1..=n {
        let asa = coef(i) - sa;
        let xsx = x[i - 1] / range - sx;
        ssa += asa * asa;
        ssx += xsx * xsx;
        sax += asa * xsx;
    }
    // W as the squared correlation of data and coefficients; w1 = 1 − W avoids rounding near 1.
    let ssassx = (ssa * ssx).sqrt();
    let w1 = (ssassx - sax) * (ssassx + sax) / (ssa * ssx);
    let w = 1.0 - w1;

    if n == 3 {
        const PI6: f64 = 6.0 / std::f64::consts::PI;
        const STQR: f64 = std::f64::consts::FRAC_PI_3;
        let w = w.clamp(0.75, 1.0);
        return Some(SwResult { w, sig: (PI6 * (w.sqrt().asin() - STQR)).clamp(0.0, 1.0) });
    }
    let mut y = w1.ln();
    let (m, s) = if n <= 11 {
        let gamma = poly(&G, an);
        if y >= gamma {
            return Some(SwResult { w, sig: 1e-99 });
        }
        y = -(gamma - y).ln();
        (poly(&C3, an), poly(&C4, an).exp())
    } else {
        let xx = an.ln();
        (poly(&C5, xx), poly(&C6, xx).exp())
    };
    Some(SwResult { w, sig: normal_cdf(-(y - m) / s) })
}
