use stat_core::dist::{f_sig, normal_cdf, normal_quantile, t_quantile, t_sig_two_tailed};
use stat_core::{describe, percentile_haverage, trimmed_mean_5};

fn load_x1() -> Vec<f64> {
    let p = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/data/skripsi60.csv");
    let s = std::fs::read_to_string(p).unwrap();
    s.lines().skip(1).map(|l| l.split(',').next().unwrap().parse().unwrap()).collect()
}

fn refs() -> serde_json::Value {
    let p = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/ref/describe_skripsi60.json");
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn close(a: f64, b: f64, tol: f64, what: &str) {
    assert!((a - b).abs() <= tol, "{what}: {a} vs {b}");
}

#[test]
fn describe_matches_reference() {
    let x = load_x1();
    let r = refs();
    let v = &r["values"];
    let g = |k: &str| v[k].as_f64().unwrap();
    let d = describe(&x).unwrap();
    assert_eq!(d.n, 60);
    close(d.mean, g("mean"), 1e-9, "mean");
    close(d.variance, g("var"), 1e-9, "var");
    close(d.sd, g("var").sqrt(), 1e-9, "sd");
    close(d.se_mean, (g("var") / 60.0).sqrt(), 1e-9, "se");
    close(d.skewness.unwrap(), g("skew"), 1e-9, "skew");
    close(d.kurtosis.unwrap(), g("kurt"), 1e-9, "kurt");
    close(d.range, d.max - d.min, 0.0, "range");
    close(d.sum, x.iter().sum::<f64>(), 1e-9, "sum");
    let mut s = x.clone();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    close(percentile_haverage(&s, 25.0), g("p25"), 1e-9, "p25");
    close(percentile_haverage(&s, 50.0), g("p50"), 1e-9, "p50");
    close(percentile_haverage(&s, 75.0), g("p75"), 1e-9, "p75");
    close(trimmed_mean_5(&s), g("trimmed5"), 1e-9, "trimmed5");
}

#[test]
fn small_n_options_and_se() {
    assert!(describe(&[]).is_none());
    let d = describe(&[5.0]).unwrap();
    assert!(d.skewness.is_none() && d.kurtosis.is_none());
    assert!(describe(&[1.0, 2.0, 4.0]).unwrap().kurtosis.is_none());
    let d = describe(&[1.0, 2.0, 4.0, 8.0, 9.0]).unwrap();
    let n = 5.0_f64;
    let se_s = (6.0 * n * (n - 1.0) / ((n - 2.0) * (n + 1.0) * (n + 3.0))).sqrt();
    close(d.se_skewness.unwrap(), se_s, 1e-12, "se skew");
    close(d.se_kurtosis.unwrap(), 2.0 * se_s * ((n * n - 1.0) / ((n - 3.0) * (n + 5.0))).sqrt(), 1e-12, "se kurt");
}

#[test]
fn trimmed_fractional_boundary() {
    // n=10 -> trim 0.5 each end: weights 0.5,1..1,0.5 ; mean = sum/9
    let x: Vec<f64> = (1..=10).map(|v| v as f64).collect();
    let expect = (0.5 * 1.0 + (2..=9).map(|v| v as f64).sum::<f64>() + 0.5 * 10.0) / 9.0;
    close(trimmed_mean_5(&x), expect, 1e-12, "frac");
}

#[test]
fn percentile_edges() {
    let s = [1.0, 2.0, 3.0, 4.0];
    close(percentile_haverage(&s, 0.0), 1.0, 0.0, "p0");
    close(percentile_haverage(&s, 100.0), 4.0, 0.0, "p100");
    close(percentile_haverage(&s, 50.0), 2.5, 1e-12, "median");
    close(percentile_haverage(&s, 25.0), 1.25, 1e-12, "p25");
}

#[test]
fn distributions() {
    close(t_sig_two_tailed(2.0, 2.0), 0.18350341907227397, 1e-12, "t sig");
    close(t_sig_two_tailed(-2.0, 2.0), 0.18350341907227397, 1e-12, "t sig neg");
    close(f_sig(3.0, 2.0, 10.0), 0.095367431640625, 1e-12, "f sig");
    close(normal_quantile(0.975), 1.959963984540054, 1e-9, "z");
    close(normal_cdf(1.959963984540054), 0.975, 1e-9, "cdf");
    close(t_quantile(0.975, 30.0), 2.0422724563012373, 1e-9, "t q30");
    close(t_quantile(0.975, 2.0), 4.302652729911275, 1e-9, "t q2");
    close(t_quantile(0.975, 10.0), 2.2281388519649385, 1e-9, "t q10");
}

#[test]
fn constant_data_has_exactly_zero_spread() {
    let d = describe(&[0.1, 0.1, 0.1]).unwrap();
    assert_eq!((d.variance, d.sd, d.se_mean), (0.0, 0.0, 0.0));
    assert!(d.skewness.is_none() && d.kurtosis.is_none());
}
