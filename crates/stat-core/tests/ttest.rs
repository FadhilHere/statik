use stat_core::ttest::{independent_t, levene_mean, one_sample_t, paired_t};

fn refs() -> serde_json::Value {
    let p = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/ref/ttest.json");
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn vec_of(v: &serde_json::Value) -> Vec<f64> {
    v.as_array().unwrap().iter().map(|e| e.as_f64().unwrap()).collect()
}

fn close(a: f64, b: f64, tol: f64, what: &str) {
    assert!((a - b).abs() <= tol, "{what}: {a} vs {b}");
}

fn sleep() -> (Vec<f64>, Vec<f64>) {
    let extra = [
        0.7, -1.6, -0.2, -1.2, -0.1, 3.4, 3.7, 0.8, 0.0, 2.0, 1.9, 0.8, 1.1, 0.1, -0.1, 4.4, 5.5, 1.6, 4.6, 3.4,
    ];
    (extra[..10].to_vec(), extra[10..].to_vec())
}

#[test]
fn sleep_paired() {
    let (a, b) = sleep();
    let r = paired_t(&a, &b, 0.95).unwrap();
    close(r.test.t, -4.0621, 1e-4, "t");
    close(r.test.df, 9.0, 0.0, "df");
    close(r.test.sig, 0.002833, 1e-6, "sig");
    close(r.test.mean_diff, -1.58, 1e-9, "mean");
    close(r.test.ci.0, -2.4598858, 1e-6, "lo");
    close(r.test.ci.1, -0.7001142, 1e-6, "hi");
}

#[test]
fn sleep_independent() {
    let (a, b) = sleep();
    let r = independent_t(&a, &b, 0.95).unwrap();
    close(r.equal.t, -1.8608, 1e-4, "t");
    close(r.equal.df, 18.0, 0.0, "df");
    close(r.equal.sig, 0.07919, 1e-5, "sig");
    close(r.welch.t, -1.8608, 1e-4, "wt");
    close(r.welch.df, 17.776, 1e-3, "wdf");
    close(r.welch.sig, 0.07939, 1e-5, "wsig");
}

#[test]
fn matches_scipy() {
    let r = refs();
    let v = &r["values"];
    let x1 = vec_of(&v["x1"]);
    let one = one_sample_t(&x1, 50.0, 0.95).unwrap();
    close(one.t, v["one_t"].as_f64().unwrap(), 1e-9, "one t");
    close(one.df, v["one_df"].as_f64().unwrap(), 0.0, "one df");
    close(one.sig, v["one_p"].as_f64().unwrap(), 1e-9, "one p");
    let ci = vec_of(&v["one_ci"]);
    close(one.ci.0 + 50.0, ci[0], 1e-9, "ci lo");
    close(one.ci.1 + 50.0, ci[1], 1e-9, "ci hi");

    let (g1, g2) = (vec_of(&v["g1"]), vec_of(&v["g2"]));
    let (f, sig, d1, d2) = levene_mean(&[&g1, &g2]);
    close(f, v["levene_f"].as_f64().unwrap(), 1e-9, "levene f");
    close(sig, v["levene_p"].as_f64().unwrap(), 1e-9, "levene p");
    assert_eq!((d1, d2), (1, g1.len() + g2.len() - 2));
    let ind = independent_t(&g1, &g2, 0.95).unwrap();
    close(ind.levene_f, f, 1e-12, "ind levene");
    close(ind.equal.t, v["eq_t"].as_f64().unwrap(), 1e-9, "eq t");
    close(ind.equal.sig, v["eq_p"].as_f64().unwrap(), 1e-9, "eq p");
    close(ind.welch.t, v["welch_t"].as_f64().unwrap(), 1e-9, "w t");
    close(ind.welch.df, v["welch_df"].as_f64().unwrap(), 1e-9, "w df");
    close(ind.welch.sig, v["welch_p"].as_f64().unwrap(), 1e-9, "w p");

    let x2 = vec_of(&v["x2"]);
    let p = paired_t(&x1, &x2, 0.95).unwrap();
    close(p.test.t, v["paired_t"].as_f64().unwrap(), 1e-9, "p t");
    close(p.test.sig, v["paired_p"].as_f64().unwrap(), 1e-9, "p p");
    close(p.corr, v["corr"].as_f64().unwrap(), 1e-9, "corr");
    close(p.corr_sig, v["corr_p"].as_f64().unwrap(), 1e-9, "corr p");
}

#[test]
fn degenerate_cases_are_none() {
    assert!(independent_t(&[1.0], &[2.0, 3.0], 0.95).is_none());
    assert!(independent_t(&[1.0, 1.0], &[2.0, 2.0], 0.95).is_none());
    assert!(one_sample_t(&[3.0, 3.0, 3.0], 1.0, 0.95).is_none());
    assert!(one_sample_t(&[3.0], 1.0, 0.95).is_none());
    assert!(paired_t(&[1.0, 2.0, 3.0], &[2.0, 3.0, 4.0], 0.95).is_none()); // constant difference
}

#[test]
fn inexact_constants_are_degenerate() {
    assert!(one_sample_t(&[0.1, 0.1, 0.1], 0.0, 0.95).is_none());
    assert!(independent_t(&[0.1, 0.1, 0.1], &[0.1, 0.1, 0.1], 0.95).is_none());
    assert!(independent_t(&[0.1, 0.1, 0.1], &[0.1, 0.2, 0.4], 0.95).is_some());
    // constant difference 0.2 (inexact) -> no test; correlation undefined for a constant variable
    assert!(paired_t(&[0.3, 0.3, 0.3], &[0.1, 0.1, 0.1], 0.95).is_none());
    let (c, s) = stat_core::paired_corr(&[0.3, 0.3, 0.3], &[0.1, 0.1, 0.1]);
    assert!(c.is_nan() && s.is_nan());
    let (c, _) = stat_core::paired_corr(&[1.0, 2.0, 3.0], &[2.0, 3.0, 5.0]);
    assert!(c > 0.9);
}
