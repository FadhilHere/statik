//! Floating-point edge cases: computed constants, large-df quantiles, near-zero fits.
use stat_core::dist::t_quantile;
use stat_core::ols::{OlsError, ols};
use stat_core::ttest::{levene_mean, paired_t};

#[test]
fn is_constant_computed_helper() {
    use stat_core::is_constant_computed;
    let post = [1.1, 2.1, 3.1, 4.1, 5.1, 6.1];
    let pre = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
    let d: Vec<f64> = post.iter().zip(&pre).map(|(a, b)| a - b).collect();
    assert!(d[0] != d[5], "repro needs bitwise-different diffs");
    assert!(is_constant_computed(&d, 6.1));
    assert!(!is_constant_computed(&[0.1, 0.2], 1.0));
    assert!(is_constant_computed(&[], 1.0));
    assert!(is_constant_computed(&[3.0, 3.0], 3.0));
}

#[test]
fn paired_inexact_constant_difference_is_none() {
    let post = [1.1, 2.1, 3.1, 4.1, 5.1, 6.1];
    let pre = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
    assert!(paired_t(&post, &pre, 0.95).is_none());
    let (c, _) = stat_core::paired_corr(&post, &pre);
    assert!(c > 0.999);
}

#[test]
fn levene_inexact_equal_deviations_is_undefined() {
    // Each group has |x - mean| equal on paper but not bitwise.
    let (f, s, _, _) = levene_mean(&[&[0.1, 0.3], &[0.5, 1.1], &[0.2, 0.7]]);
    assert!(f.is_nan() && s.is_nan(), "F {f}, sig {s}");
}

fn refs() -> serde_json::Value {
    let p = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/ref/t_quantile_large_df.json");
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

#[test]
fn t_quantile_large_df_matches_scipy() {
    let r = &refs()["values"];
    let dfs = r["df"].as_array().unwrap();
    let ps = r["p"].as_array().unwrap();
    for (i, df) in dfs.iter().enumerate() {
        for (j, p) in ps.iter().enumerate() {
            let (df, p) = (df.as_f64().unwrap(), p.as_f64().unwrap());
            let want = r["q"][i][j].as_f64().unwrap();
            let got = t_quantile(p, df);
            assert!(((got - want) / want).abs() <= 1e-9, "df {df} p {p}: {got} vs {want}");
        }
    }
}

#[test]
fn zero_slope_regression_is_clean() {
    let y = [0.1, 0.2, 0.3, 0.2, 0.1];
    let x = [0.1, 0.2, 0.3, 0.4, 0.5];
    let f = ols(&y, &[&x]).unwrap();
    assert!(f.r2 >= 0.0 && f.r.is_finite(), "r2 {} r {}", f.r2, f.r);
    assert!(f.r.abs() < 1e-6);
    assert!((f.adj_r2 - (1.0 - (1.0 - f.r2) * 4.0 / 3.0)).abs() < 1e-15);
    assert!(f.z_predicted.iter().all(|v| v.is_nan()), "{:?}", f.z_predicted);
}

#[test]
fn large_offset_variables_are_not_constant() {
    let y = [1_000_000.0, 1_000_001.0, 1_000_002.0, 1_000_000.5];
    let x = [1.0, 2.0, 3.0, 1.5];
    let f = ols(&y, &[&x]).expect("dependent with large offset is not constant");
    assert!((f.b[1] - 1.0).abs() < 1e-9);
    let y = [1.0, 2.0, 2.5, 4.0];
    let x = [5_000_000.0, 5_000_001.0, 5_000_002.0, 5_000_003.0];
    let f = ols(&y, &[&x]).unwrap();
    assert!(f.excluded.is_empty(), "excluded {:?}", f.excluded);
    assert!(!matches!(ols(&[1_000_000.0, 1_000_001.0, 1_000_002.0], &[&[1.0, 2.0, 4.0]]), Err(OlsError::ZeroVarianceDependent)));
}

#[test]
fn t_quantile_continuous_across_method_switch() {
    // statrs below df 1000, Cornish-Fisher from 1000: both must agree at the seam.
    for p in [0.025, 0.95, 0.975, 0.995] {
        let (a, b) = (t_quantile(p, 1000.0 - 1e-9), t_quantile(p, 1000.0));
        assert!(((a - b) / b).abs() < 1e-9, "p {p}: {a} vs {b}");
    }
    assert_eq!(t_quantile(1.0, 1e6), f64::INFINITY);
    assert_eq!(t_quantile(0.5, 1e6), 0.0);
}
