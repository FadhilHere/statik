use stat_core::normality::{lilliefors, shapiro_wilk};

fn refs() -> serde_json::Value {
    let p = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/ref/normality.json");
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn close(a: f64, b: f64, tol: f64, what: &str) {
    assert!((a - b).abs() <= tol, "{what}: {a} vs {b}");
}

const SAMPLES: &[&str] =
    &["x1", "resid", "sleep_extra", "sleep_3", "sleep_4", "sleep_5", "sleep_10", "skew30", "skew150"];

#[test]
fn shapiro_wilk_matches_scipy() {
    let r = refs();
    let v = &r["values"];
    for &name in SAMPLES {
        let x: Vec<f64> = v[name].as_array().unwrap().iter().map(|e| e.as_f64().unwrap()).collect();
        let sw = shapiro_wilk(&x).unwrap_or_else(|| panic!("{name}: None"));
        close(sw.w, v[format!("{name}_sw_w")].as_f64().unwrap(), 1e-6, &format!("{name} W"));
        close(sw.sig, v[format!("{name}_sw_p")].as_f64().unwrap(), 1e-6, &format!("{name} p"));
    }
}

#[test]
fn lilliefors_matches_statsmodels() {
    let r = refs();
    let v = &r["values"];
    let mut checked = 0;
    for &name in SAMPLES {
        let x: Vec<f64> = v[name].as_array().unwrap().iter().map(|e| e.as_f64().unwrap()).collect();
        let Some(d_ref) = v[format!("{name}_lf_d")].as_f64() else { continue };
        let p_ref = v[format!("{name}_lf_p")].as_f64().unwrap();
        let ks = lilliefors(&x).unwrap_or_else(|| panic!("{name}: None"));
        close(ks.d, d_ref, 1e-9, &format!("{name} D"));
        close(ks.d, ks.d_pos.max(-ks.d_neg), 1e-15, &format!("{name} D = max(D+, |D-|)"));
        assert!(ks.d_pos >= 0.0 && ks.d_neg <= 0.0, "{name}: signs");
        if p_ref < 0.1 {
            close(ks.sig, p_ref, 1e-4, &format!("{name} sig"));
            assert!((ks.sig - p_ref).abs() <= 1e-8 * p_ref, "{name} sig relative: {} vs {p_ref}", ks.sig);
            assert!(!ks.sig_is_lower_bound);
        } else if p_ref >= 0.2 {
            assert_eq!(ks.sig, 0.2, "{name}");
            assert!(ks.sig_is_lower_bound, "{name}");
        }
        checked += 1;
    }
    assert_eq!(checked, 7);
}

#[test]
fn too_small_or_degenerate_samples() {
    assert!(shapiro_wilk(&[1.0, 2.0]).is_none());
    assert!(shapiro_wilk(&[1.0; 10]).is_none(), "zero range");
    assert!(shapiro_wilk(&(0..5001).map(f64::from).collect::<Vec<_>>()).is_none());
    assert!(shapiro_wilk(&(0..5000).map(f64::from).collect::<Vec<_>>()).is_some());
    assert!(lilliefors(&[1.0, 2.0, 3.0, 4.0]).is_none());
    assert!(lilliefors(&[3.0; 6]).is_none(), "sd = 0");
    assert!(lilliefors(&[1.0, 2.0, 3.0, 4.0, 9.0]).is_some());
}

#[test]
fn shapiro_wilk_unsorted_input_and_n3() {
    let a = shapiro_wilk(&[3.0, 1.0, 2.5, 7.0, 4.2]).unwrap();
    let b = shapiro_wilk(&[1.0, 2.5, 3.0, 4.2, 7.0]).unwrap();
    assert_eq!(a.w, b.w);
    // n = 3 equally spaced: W = 1, exact p = 1.
    let s = shapiro_wilk(&[1.0, 2.0, 3.0]).unwrap();
    close(s.w, 1.0, 1e-12, "W");
    close(s.sig, 1.0, 1e-9, "p");
    // n = 3 most extreme: W = 0.75, p = 0.
    let s = shapiro_wilk(&[0.0, 0.0, 1.0]).unwrap();
    close(s.w, 0.75, 1e-12, "W");
    close(s.sig, 0.0, 1e-9, "p");
}
