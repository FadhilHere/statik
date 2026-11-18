//! Pure statistical math on slices (no dependency on the dataset crate).
pub mod anova;
pub mod describe;
pub mod dist;
pub mod normality;
pub mod ols;
pub mod runs;
pub mod ttest;

pub use describe::{Describe, describe, percentile_haverage, trimmed_mean_5};
pub use dist::{f_sig, normal_cdf, normal_quantile, t_quantile, t_sig_two_tailed};
pub use normality::{KsResult, SwResult, lilliefors, shapiro_wilk};
pub use ols::{CandidateStats, OlsError, OlsFit, candidate_stats, ols};
pub use ttest::{paired_corr, IndResult, PairedResult, TResult, independent_t, levene_mean, one_sample_t, paired_t};
pub use anova::{OneWay, oneway};
pub use runs::{RunsResult, runs_test_median};
