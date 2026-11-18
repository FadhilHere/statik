#![allow(clippy::needless_range_loop)]
use stat_core::dist::{f_sig, t_quantile, t_sig_two_tailed};
use stat_core::ols::{OlsError, ols};

fn read_csv(name: &str) -> (Vec<String>, Vec<Vec<f64>>) {
    let p = format!("{}/../../fixtures/data/{name}", env!("CARGO_MANIFEST_DIR"));
    let s = std::fs::read_to_string(p).unwrap();
    let mut lines = s.lines();
    let head: Vec<String> = lines.next().unwrap().split(',').map(str::to_string).collect();
    let mut cols = vec![Vec::new(); head.len()];
    for l in lines {
        for (c, cell) in cols.iter_mut().zip(l.split(',')) {
            c.push(cell.parse().unwrap_or(f64::NAN));
        }
    }
    (head, cols)
}

fn col<'a>(h: &[String], c: &'a [Vec<f64>], name: &str) -> &'a [f64] {
    &c[h.iter().position(|x| x == name).unwrap()]
}

fn refs() -> serde_json::Value {
    let p = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/ref/ols_skripsi60.json");
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn rel(a: f64, b: f64, tol: f64, what: &str) {
    let d = (a - b).abs() / b.abs().max(1e-300);
    assert!(d <= tol, "{what}: got {a}, expected {b} (rel diff {d:e})");
}

fn arr(v: &serde_json::Value, k: &str) -> Vec<f64> {
    v["values"][k].as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect()
}

fn num(v: &serde_json::Value, k: &str) -> f64 {
    v["values"][k].as_f64().unwrap()
}

#[test]
fn longley_nist_certified() {
    let (h, c) = read_csv("longley.csv");
    let y = col(&h, &c, "y");
    let xs: Vec<&[f64]> = ["x1", "x2", "x3", "x4", "x5", "x6"].iter().map(|n| col(&h, &c, n)).collect();
    let f = ols(y, &xs).unwrap();
    let cert = [
        -3482258.63459582,
        15.0618722713733,
        -0.358191792925910e-1,
        -2.02022980381683,
        -1.03322686717359,
        -0.511041056535807e-1,
        1829.15146461355,
    ];
    assert!(f.excluded.is_empty());
    assert_eq!(f.b.len(), 7);
    for (i, (&g, &e)) in f.b.iter().zip(&cert).enumerate() {
        rel(g, e, 1e-7, &format!("b[{i}]"));
    }
    rel(f.r2, 0.995479004577296, 1e-7, "r2");
    rel(f.se_est, 304.854073561965, 1e-7, "se_est");
    assert_eq!((f.df_reg, f.df_res), (6, 9));
}

#[test]
fn skripsi60_matches_statsmodels() {
    let (h, c) = read_csv("skripsi60.csv");
    let r = refs();
    let y = col(&h, &c, "y");
    let xs: Vec<&[f64]> = ["x1", "x2", "x3"].iter().map(|n| col(&h, &c, n)).collect();
    let f = ols(y, &xs).unwrap();
    let tol = 1e-9;
    for (k, got) in [("params", &f.b), ("bse", &f.se), ("tvalues", &f.t), ("pvalues", &f.sig)] {
        let exp = arr(&r, k);
        assert_eq!(got.len(), exp.len(), "{k} len");
        for (i, (&g, &e)) in got.iter().zip(&exp).enumerate() {
            rel(g, e, if k == "pvalues" { 1e-6 } else { tol }, &format!("{k}[{i}]"));
        }
    }
    rel(f.r2, num(&r, "rsquared"), tol, "r2");
    rel(f.r, num(&r, "rsquared").sqrt(), tol, "r");
    rel(f.adj_r2, num(&r, "rsquared_adj"), tol, "adj_r2");
    rel(f.f, num(&r, "fvalue"), tol, "f");
    rel(f.f_sig, num(&r, "f_pvalue"), 1e-6, "f_sig");
    rel(f.durbin_watson, num(&r, "durbin_watson"), tol, "dw");
    let vif = arr(&r, "vif");
    assert!(f.vif[0].is_nan() && f.tolerance[0].is_nan() && f.beta[0].is_nan());
    for j in 0..3 {
        rel(f.vif[j + 1], vif[j], 1e-8, &format!("vif[{j}]"));
        rel(f.tolerance[j + 1], 1.0 / vif[j], 1e-8, &format!("tol[{j}]"));
    }
    let sres = arr(&r, "sresid");
    assert_eq!(f.studentized_residuals.len(), 60);
    for (i, (&g, &e)) in f.studentized_residuals.iter().zip(&sres).enumerate() {
        rel(g, e, 1e-8, &format!("sresid[{i}]"));
    }
    // sums of squares, residuals, predicted, z-scores
    assert_eq!((f.df_reg, f.df_res), (3, 56));
    rel(f.ss_reg / (f.ss_reg + f.ss_res), f.r2, 1e-12, "ss decomposition");
    for i in 0..60 {
        rel(f.predicted[i] + f.residuals[i], y[i], 1e-12, "yhat + e = y");
        rel(f.z_residuals[i], f.residuals[i] / f.se_est, 1e-12, "zresid");
    }
    let mz: f64 = f.z_predicted.iter().sum::<f64>() / 60.0;
    let vz: f64 = f.z_predicted.iter().map(|z| (z - mz) * (z - mz)).sum::<f64>() / 59.0;
    assert!(mz.abs() < 1e-12 && (vz - 1.0).abs() < 1e-12);
    // beta = b * sd(x)/sd(y)
    let sd = |v: &[f64]| {
        let m = v.iter().sum::<f64>() / v.len() as f64;
        (v.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / (v.len() as f64 - 1.0)).sqrt()
    };
    for j in 0..3 {
        rel(f.beta[j + 1], f.b[j + 1] * sd(xs[j]) / sd(y), 1e-12, "beta");
    }
    // collinearity diagnostics: 4 dimensions, descending, CI[0] = 1, proportions sum to 1 per coefficient
    assert_eq!(f.eigenvalues.len(), 4);
    assert!(f.eigenvalues.windows(2).all(|w| w[0] >= w[1]));
    rel(f.eigenvalues.iter().sum::<f64>(), 4.0, 1e-12, "trace of scaled X'X");
    rel(f.condition_index[0], 1.0, 1e-12, "ci0");
    for k in 0..4 {
        rel(f.condition_index[k], (f.eigenvalues[0] / f.eigenvalues[k]).sqrt(), 1e-12, "ci");
    }
    assert_eq!(f.variance_proportions.len(), 4); // [dimension][coefficient]
    for j in 0..4 {
        let s: f64 = f.variance_proportions.iter().map(|row| row[j]).sum();
        rel(s, 1.0, 1e-12, "proportions sum");
    }
}

#[test]
fn excludes_collinear_predictor() {
    let (h, c) = read_csv("edge_cases.csv");
    let f = ols(col(&h, &c, "y"), &[col(&h, &c, "x1"), col(&h, &c, "x1dbl")]).unwrap();
    assert_eq!(f.excluded, vec![1]);
    assert_eq!(f.b.len(), 2);
    assert_eq!(f.se.len(), 2);
    assert_eq!(f.vif.len(), 2);
    assert_eq!(f.df_reg, 1);
    assert_eq!(f.df_res, 10);
}

#[test]
fn too_few_cases() {
    let y = [1.0, 2.0, 4.0];
    let a = [1.0, 2.0, 3.0];
    let b = [3.0, 1.0, 2.0];
    let c = [5.0, 5.0, 1.0];
    assert!(matches!(ols(&y, &[&a, &b, &c]), Err(OlsError::TooFewCases)));
}

#[test]
fn dist_invalid_df_returns_nan() {
    assert!(t_sig_two_tailed(1.0, 0.0).is_nan());
    assert!(t_sig_two_tailed(1.0, f64::NAN).is_nan());
    assert!(t_quantile(0.5, -1.0).is_nan());
    assert!(f_sig(1.0, 0.0, 5.0).is_nan());
    assert!(f_sig(1.0, 2.0, 0.0).is_nan());
    assert!(f_sig(f64::NAN, 2.0, 5.0).is_nan());
}

#[test]
fn candidate_stats_match_full_model() {
    use stat_core::ols::candidate_stats;
    let (h, c) = read_csv("skripsi60.csv");
    let r = refs();
    let y = col(&h, &c, "y");
    let (x1, x2, x3) = (col(&h, &c, "x1"), col(&h, &c, "x2"), col(&h, &c, "x3"));
    let full = ols(y, &[x1, x2, x3]).unwrap();
    let s = candidate_stats(y, &[x1, x2], x3);
    rel(s.t, arr(&r, "tvalues")[3], 1e-9, "t");
    rel(s.sig, arr(&r, "pvalues")[3], 1e-6, "sig");
    rel(s.beta_in, full.beta[3], 1e-9, "beta in");
    rel(s.tolerance, full.tolerance[3], 1e-9, "tolerance");
    rel(s.vif, full.vif[3], 1e-9, "vif");
    let min_tol = full.tolerance[1..].iter().cloned().fold(f64::INFINITY, f64::min);
    rel(s.min_tolerance, min_tol, 1e-9, "min tolerance");
    // partial correlation: t = r*sqrt(df/(1-r^2)) with df = n - 4
    let pr = s.partial;
    rel(pr * (56.0 / (1.0 - pr * pr)).sqrt(), s.t, 1e-9, "partial");
}

#[test]
fn candidate_stats_perfectly_collinear() {
    use stat_core::ols::candidate_stats;
    let (h, c) = read_csv("edge_cases.csv");
    let s = candidate_stats(col(&h, &c, "y"), &[col(&h, &c, "x1")], col(&h, &c, "x1dbl"));
    assert!(s.tolerance < 1e-4);
    assert!(s.beta_in.is_nan() && s.t.is_nan() && s.sig.is_nan() && s.partial.is_nan());
}
