//! Distribution helpers backed by `statrs`.
//!
//! Invalid inputs (non-positive or NaN degrees of freedom, NaN statistic) yield `f64::NAN`
//! instead of panicking.
use statrs::distribution::{Continuous, ContinuousCDF, FisherSnedecor, Normal, StudentsT};

fn valid_df(df: f64) -> bool {
    df > 0.0 && !df.is_nan()
}

/// Two-tailed p-value for a t statistic: P(|T| > |t|).
pub fn t_sig_two_tailed(t: f64, df: f64) -> f64 {
    if t.is_nan() || !valid_df(df) {
        return f64::NAN;
    }
    match StudentsT::new(0.0, 1.0, df) {
        Ok(d) => (2.0 * d.sf(t.abs())).min(1.0),
        Err(_) => f64::NAN,
    }
}

/// Inverse CDF of Student's t.
///
/// statrs' `inverse_cdf` (and the CDF it inverts) loses accuracy at large df (rel. error ~1e-6
/// at df 1e5, ~6e-5 at df 1e6), so Newton polishing on that CDF cannot help there. For df >= 1000 the
/// Cornish-Fisher expansion in 1/df (Abramowitz & Stegun 26.7.5, terms through df^-4) is used
/// instead; its truncation error there is ~1e-13 relative or better for p in [1e-5, 1 - 1e-5].
pub fn t_quantile(p: f64, df: f64) -> f64 {
    if !(0.0..=1.0).contains(&p) || !valid_df(df) {
        return f64::NAN;
    }
    if df >= 1000.0 {
        let z = normal_quantile(p);
        if !z.is_finite() {
            return z;
        }
        let z2 = z * z;
        let g1 = z * (z2 + 1.0) / 4.0;
        let g2 = z * ((5.0 * z2 + 16.0) * z2 + 3.0) / 96.0;
        let g3 = z * (((3.0 * z2 + 19.0) * z2 + 17.0) * z2 - 15.0) / 384.0;
        let g4 = z * ((((79.0 * z2 + 776.0) * z2 + 1482.0) * z2 - 1920.0) * z2 - 945.0) / 92160.0;
        return z + (g1 + (g2 + (g3 + g4 / df) / df) / df) / df;
    }
    let Ok(d) = StudentsT::new(0.0, 1.0, df) else { return f64::NAN };
    // Below df 1000 the CDF is accurate but inverse_cdf can stop ~1e-9 short: polish with Newton.
    let mut q = d.inverse_cdf(p);
    for _ in 0..2 {
        let dens = d.pdf(q);
        if !q.is_finite() || dens.is_nan() || dens <= 0.0 {
            break;
        }
        q -= (d.cdf(q) - p) / dens;
    }
    q
}

/// Upper-tail probability P(F > f).
pub fn f_sig(f: f64, df1: f64, df2: f64) -> f64 {
    if f.is_nan() || !valid_df(df1) || !valid_df(df2) {
        return f64::NAN;
    }
    FisherSnedecor::new(df1, df2).map_or(f64::NAN, |d| d.sf(f))
}

/// Standard normal CDF.
pub fn normal_cdf(z: f64) -> f64 {
    Normal::new(0.0, 1.0).unwrap().cdf(z)
}

/// Standard normal inverse CDF.
pub fn normal_quantile(p: f64) -> f64 {
    Normal::new(0.0, 1.0).unwrap().inverse_cdf(p)
}
