use stat_core::{oneway, runs_test_median};

fn refs(case: &str) -> serde_json::Value {
    let p = format!("{}/../../fixtures/ref/{case}.json", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn vec_of(v: &serde_json::Value) -> Vec<f64> {
    v.as_array().unwrap().iter().map(|e| e.as_f64().unwrap()).collect()
}

fn close(a: f64, b: f64, tol: f64, what: &str) {
    assert!((a - b).abs() <= tol, "{what}: {a} vs {b}");
}

#[test]
fn oneway_matches_scipy() {
    let r = refs("anova");
    let v = &r["values"];
    let (g1, g2) = (vec_of(&v["g1"]), vec_of(&v["g2"]));
    let a = oneway(&[&g1, &g2]).unwrap();
    close(a.f, v["f"].as_f64().unwrap(), 1e-9, "F");
    close(a.sig, v["p"].as_f64().unwrap(), 1e-10, "p");
    close(a.ss_between, v["ssb"].as_f64().unwrap(), 1e-8, "ssb");
    close(a.ss_within, v["ssw"].as_f64().unwrap(), 1e-8, "ssw");
    assert_eq!((a.df_between, a.df_within), (1, 58));
}

#[test]
fn oneway_degenerate() {
    assert!(oneway(&[&[1.0, 2.0, 3.0][..]]).is_none());
    assert!(oneway(&[&[1.0, 2.0][..], &[]]).is_none());
    assert!(oneway(&[&[1.0][..], &[2.0]]).is_none()); // df within = 0
    let a = oneway(&[&[2.0, 2.0][..], &[5.0, 5.0]]).unwrap();
    assert_eq!(a.ss_within, 0.0);
    assert!(a.f.is_nan() && a.sig.is_nan());
}

#[test]
fn runs_matches_reference() {
    let r = refs("runs");
    let v = &r["values"];
    let res = vec_of(&v["resid"]);
    let o = runs_test_median(&res).unwrap();
    close(o.test_value, v["median"].as_f64().unwrap(), 1e-9, "median");
    assert_eq!(o.n_below, v["n_below"].as_u64().unwrap() as usize);
    assert_eq!(o.n_at_or_above, v["n_above"].as_u64().unwrap() as usize);
    assert_eq!(o.runs, v["runs"].as_u64().unwrap() as usize);
    close(o.z, v["z"].as_f64().unwrap(), 1e-9, "z");
    close(o.sig, v["p"].as_f64().unwrap(), 1e-9, "p");
}

#[test]
fn runs_constant_is_none() {
    assert!(runs_test_median(&[3.0, 3.0, 3.0, 3.0]).is_none());
    assert!(runs_test_median(&[]).is_none());
}

#[test]
fn runs_alternating_and_continuity() {
    // median 4.5: below = 1,2,3,4 ; alternate low/high => runs = 8
    let x = [1.0, 5.0, 2.0, 6.0, 3.0, 7.0, 4.0, 8.0];
    let o = runs_test_median(&x).unwrap();
    assert_eq!((o.n_below, o.n_at_or_above, o.runs), (4, 4, 8));
    // mu = 5, var = 2*16*(32-8)/(64*7) = 1.7142857
    let z = (8.0 - 5.0 - 0.5) / (2.0f64 * 16.0 * 24.0 / (64.0 * 7.0)).sqrt();
    close(o.z, z, 1e-12, "z");
}

#[test]
fn levene_constant_inexact_groups_is_nan() {
    let (f, sig, df1, df2) = stat_core::levene_mean(&[&[0.1, 0.1, 0.1][..], &[0.2, 0.2, 0.2]]);
    assert!(f.is_nan() && sig.is_nan());
    assert_eq!((df1, df2), (1, 4));
}
