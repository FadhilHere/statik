//! Distribution helpers backed by `statrs`.
//!
//! Invalid inputs (non-positive or NaN degrees of freedom, NaN statistic) yield `f64::NAN`
//! instead of panicking.
use statrs::distribution::{ContinuousCDF, FisherSnedecor, Normal, StudentsT};

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
pub fn t_quantile(p: f64, df: f64) -> f64 {
    if !(0.0..=1.0).contains(&p) || !valid_df(df) {
        return f64::NAN;
    }
    StudentsT::new(0.0, 1.0, df).map_or(f64::NAN, |d| d.inverse_cdf(p))
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
