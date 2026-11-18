use std::path::PathBuf;
use std::process::{Command, Output};

fn csv() -> String {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/data/skripsi60.csv");
    p.to_string_lossy().into_owned()
}

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_statik-cli")).args(args).output().unwrap()
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

#[test]
fn regression_text() {
    let f = csv();
    let o = run(&["regression", &f, "--dep", "y", "--indep", "x1,x2,x3"]);
    assert_eq!(o.status.code(), Some(0));
    let s = stdout(&o);
    for k in ["Model Summary", "ANOVA", "Coefficients"] {
        assert!(s.contains(k), "missing {k}");
    }
}

#[test]
fn regression_html() {
    let f = csv();
    let out = std::env::temp_dir().join("statik_cli_test_out.html");
    let _ = std::fs::remove_file(&out);
    let o = run(&["regression", &f, "--dep", "y", "--indep", "x1,x2,x3", "--html", out.to_str().unwrap()]);
    assert_eq!(o.status.code(), Some(0));
    let h = std::fs::read_to_string(&out).unwrap();
    assert!(h.contains("<table"));
    assert!(h.starts_with("<!doctype html>"));
}

#[test]
fn unknown_variable() {
    let f = csv();
    let o = run(&["regression", &f, "--dep", "y", "--indep", "foo"]);
    assert_eq!(o.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&o.stderr).contains("unknown variable"));
}

#[test]
fn usage_errors() {
    assert_eq!(run(&[]).status.code(), Some(2));
    assert_eq!(run(&["--help"]).status.code(), Some(0));
    assert_eq!(run(&["bogus", "x.csv"]).status.code(), Some(2));
    let f = csv();
    assert_eq!(run(&["ttest-one", &f, "--var", "y", "--value", "abc"]).status.code(), Some(2));
    assert_eq!(run(&["ks", "no_such_file.csv", "--var", "y"]).status.code(), Some(1));
}

#[test]
fn other_subcommands() {
    let f = csv();
    let cases: Vec<Vec<&str>> = vec![
        vec!["ttest-ind", &f, "--var", "y", "--group", "grp", "--groups", "1,2"],
        vec!["ttest-paired", &f, "--pair", "x1,x2"],
        vec!["ttest-one", &f, "--var", "y", "--value", "50"],
        vec!["explore", &f, "--var", "y"],
        vec!["ks", &f, "--var", "y"],
        vec!["runs", &f, "--var", "y"],
        vec!["oneway", &f, "--dep", "y", "--factor", "grp"],
    ];
    for c in cases {
        let o = run(&c);
        assert_eq!(o.status.code(), Some(0), "{c:?}: {}", String::from_utf8_lossy(&o.stderr));
        assert!(!stdout(&o).is_empty());
    }
}
