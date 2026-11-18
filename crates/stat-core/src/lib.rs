//! Pure statistical math on slices (no dependency on the dataset crate).
pub mod describe;
pub mod dist;

pub use describe::{Describe, describe, percentile_haverage, trimmed_mean_5};
pub use dist::{f_sig, normal_cdf, normal_quantile, t_quantile, t_sig_two_tailed};
