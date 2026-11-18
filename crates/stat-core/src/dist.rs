//! Distribution helpers backed by `statrs`.
use statrs::distribution::{ContinuousCDF, FisherSnedecor, Normal, StudentsT};

/// Two-tailed p-value for a t statistic: P(|T| > |t|).
pub fn t_sig_two_tailed(t: f64, df: f64) -> f64 {
    let d = StudentsT::new(0.0, 1.0, df).expect("valid df");
    (2.0 * d.sf(t.abs())).min(1.0)
}

/// Inverse CDF of Student's t.
pub fn t_quantile(p: f64, df: f64) -> f64 {
    StudentsT::new(0.0, 1.0, df).expect("valid df").inverse_cdf(p)
}

/// Upper-tail probability P(F > f).
pub fn f_sig(f: f64, df1: f64, df2: f64) -> f64 {
    FisherSnedecor::new(df1, df2).expect("valid df").sf(f)
}

/// Standard normal CDF.
pub fn normal_cdf(z: f64) -> f64 {
    Normal::new(0.0, 1.0).unwrap().cdf(z)
}

/// Standard normal inverse CDF.
pub fn normal_quantile(p: f64) -> f64 {
    Normal::new(0.0, 1.0).unwrap().inverse_cdf(p)
}
