use dataset::{Column, Dataset, Measure, MissingSpec, OrderedValue, VarId, VarType, Variable};
use sav_io::{read_sav, write_sav, SavError};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fixture(name: &str) -> PathBuf {
    root().join("fixtures/sav").join(name)
}

fn tmp(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("sav-io");
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

fn expected() -> Value {
    let text = std::fs::read_to_string(root().join("fixtures/ref/sav_basic.json")).unwrap();
    serde_json::from_str(&text).unwrap()
}

fn ids(ds: &Dataset) -> Vec<VarId> {
    (0..ds.n_vars()).map(VarId).collect()
}

fn measure_name(m: Measure) -> &'static str {
    match m {
        Measure::Scale => "scale",
        Measure::Ordinal => "ordinal",
        Measure::Nominal => "nominal",
    }
}

fn opt_f64(v: &Value) -> Option<f64> {
    v.as_f64()
}

fn assert_bits(ctx: &str, got: f64, want: Option<f64>) {
    match want {
        None => assert!(got.is_nan(), "{ctx}: expected NaN, got {got}"),
        Some(w) => assert_eq!(got.to_bits(), w.to_bits(), "{ctx}: got {got:?}, want {w:?}"),
    }
}

/// Compares a dataset with one `files[...]` entry of sav_basic.json.
fn assert_matches_json(file: &str, ds: &Dataset, exp: &Value) {
    let vars = exp["variables"].as_array().unwrap();
    assert_eq!(ds.n_vars(), vars.len(), "{file}: variable count");
    assert_eq!(ds.n_cases() as u64, exp["n_cases"].as_u64().unwrap(), "{file}: case count");
    for (id, ev) in ids(ds).into_iter().zip(vars) {
        let v = ds.var(id);
        let ctx = format!("{file}/{}", v.name);
        assert_eq!(v.name, ev["name"].as_str().unwrap(), "{ctx}: name");
        assert_eq!(v.label.as_deref(), ev["label"].as_str(), "{ctx}: label");
        assert_eq!(v.width as u64, ev["width"].as_u64().unwrap(), "{ctx}: width");
        assert_eq!(v.decimals as u64, ev["decimals"].as_u64().unwrap(), "{ctx}: decimals");
        assert_eq!(measure_name(v.measure), ev["measure"].as_str().unwrap(), "{ctx}: measure");
        let want_labels: BTreeMap<OrderedValue, String> = ev["value_labels"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| (OrderedValue::new(p[0].as_f64().unwrap()), p[1].as_str().unwrap().to_string()))
            .collect();
        assert_eq!(v.value_labels, want_labels, "{ctx}: value labels");
        let want_missing = match &ev["missing"] {
            Value::Null => MissingSpec::None,
            m if m["kind"] == "discrete" => {
                MissingSpec::Discrete(m["values"].as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect())
            }
            m => MissingSpec::Range {
                lo: m["lo"].as_f64().unwrap(),
                hi: m["hi"].as_f64().unwrap(),
                discrete: m["discrete"].as_f64(),
            },
        };
        assert_eq!(v.missing, want_missing, "{ctx}: missing");
        let evals = ev["values"].as_array().unwrap();
        match (ev["type"].as_str().unwrap(), ds.column(id)) {
            ("numeric", Column::Numeric(col)) => {
                assert_eq!(v.var_type, VarType::Numeric, "{ctx}: type");
                for (i, (g, w)) in col.iter().zip(evals).enumerate() {
                    assert_bits(&format!("{ctx}[{i}]"), *g, opt_f64(w));
                }
            }
            ("string", Column::Text(col)) => {
                assert_eq!(v.var_type, VarType::String { width: v.width }, "{ctx}: type");
                let want: Vec<&str> = evals.iter().map(|x| x.as_str().unwrap()).collect();
                assert_eq!(col.iter().map(String::as_str).collect::<Vec<_>>(), want, "{ctx}: values");
            }
            (t, _) => panic!("{ctx}: column type does not match expected {t}"),
        }
    }
}

const FIXTURES: [&str; 6] =
    ["basic.sav", "uncompressed.sav", "long_names.sav", "long_string.sav", "cp1252.sav", "bigendian.sav"];

#[test]
fn reads_every_fixture_like_the_reference_json() {
    let exp = expected();
    for file in FIXTURES {
        let ds = read_sav(&fixture(file)).unwrap_or_else(|e| panic!("{file}: {e}"));
        assert_matches_json(file, &ds, &exp["files"][file]);
    }
}

#[test]
fn windows_1252_text_is_decoded() {
    let ds = read_sav(&fixture("cp1252.sav")).unwrap();
    let kota = ds.find("kota").unwrap();
    assert_eq!(ds.var(kota).label.as_deref(), Some("Kota asal (é)"));
    match ds.column(kota) {
        Column::Text(v) => assert_eq!(v[0], "Café"),
        Column::Numeric(_) => panic!("kota should be a string"),
    }
}

#[test]
fn negative_zero_is_normalised() {
    for file in ["basic.sav", "uncompressed.sav"] {
        let ds = read_sav(&fixture(file)).unwrap();
        let skor = ds.numeric(ds.find("skor").unwrap());
        assert_eq!(skor[1].to_bits(), 0.0f64.to_bits(), "{file}");
    }
}

#[test]
fn round_trip_is_identical() {
    let exp = expected();
    for file in FIXTURES {
        let first = read_sav(&fixture(file)).unwrap();
        let out = tmp(&format!("roundtrip_{file}"));
        write_sav(&first, &out).unwrap_or_else(|e| panic!("{file}: {e}"));
        let second = read_sav(&out).unwrap_or_else(|e| panic!("{file} (rewritten): {e}"));
        assert_matches_json(file, &second, &exp["files"][file]);
    }
}

/// Short names must be unique even when long names share their first 8 characters.
#[test]
fn writes_unique_short_names() {
    let mut ds = Dataset::new();
    for name in ["pendapatan_a", "pendapatan_b", "PENDAPATAN", "pendapat", "x"] {
        ds.add_variable(Variable::numeric(name), Column::Numeric(vec![1.0])).unwrap();
    }
    let out = tmp("short_names.sav");
    write_sav(&ds, &out).unwrap();
    let back = read_sav(&out).unwrap();
    let names: Vec<_> = ids(&back).into_iter().map(|id| back.var(id).name.clone()).collect();
    assert_eq!(names, ["pendapatan_a", "pendapatan_b", "PENDAPATAN", "pendapat", "x"]);
}

fn venv_python() -> Option<PathBuf> {
    let p = root().join("tools/reference/.venv/Scripts/python.exe");
    if p.exists() {
        return Some(p);
    }
    let p = root().join("tools/reference/.venv/bin/python");
    p.exists().then_some(p)
}

fn pyreadstat_json(path: &Path) -> Option<Value> {
    let Some(py) = venv_python() else {
        println!("SKIPPED: tools/reference/.venv python not found; pyreadstat compatibility not checked");
        return None;
    };
    let out = Command::new(py)
        .arg("-I")
        .arg(root().join("tools/reference/check_sav.py"))
        .arg(path)
        .output()
        .expect("run check_sav.py");
    assert!(out.status.success(), "check_sav.py failed: {}", String::from_utf8_lossy(&out.stderr));
    Some(serde_json::from_slice(&out.stdout).unwrap())
}

/// Files written by Statik are read by an independent implementation (ReadStat via pyreadstat).
#[test]
fn pyreadstat_reads_statik_output() {
    for file in FIXTURES {
        let ds = read_sav(&fixture(file)).unwrap();
        let out = tmp(&format!("pyreadstat_{file}"));
        write_sav(&ds, &out).unwrap();
        let Some(got) = pyreadstat_json(&out) else { return };
        assert_eq!(got["encoding"], "UTF-8", "{file}");
        assert_eq!(got["n_cases"].as_u64().unwrap(), ds.n_cases() as u64, "{file}");
        let gv = got["variables"].as_array().unwrap();
        assert_eq!(gv.len(), ds.n_vars(), "{file}");
        for (id, g) in ids(&ds).into_iter().zip(gv) {
            let v = ds.var(id);
            let ctx = format!("{file}/{}", v.name);
            assert_eq!(g["name"], v.name.as_str(), "{ctx}");
            assert_eq!(g["label"].as_str(), v.label.as_deref(), "{ctx}: label");
            let fmt = match v.var_type {
                VarType::Numeric => format!("F{}.{}", v.width, v.decimals),
                VarType::String { width } => format!("A{width}"),
            };
            assert_eq!(g["format"], fmt.as_str(), "{ctx}: format");
            assert_eq!(g["measure"], measure_name(v.measure), "{ctx}: measure");
            let labels: Vec<(f64, String)> = g["value_labels"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| (p[0].as_f64().unwrap(), p[1].as_str().unwrap().to_string()))
                .collect();
            let want: Vec<(f64, String)> = v.value_labels.iter().map(|(k, l)| (k.0, l.clone())).collect();
            assert_eq!(labels, want, "{ctx}: value labels");
            let ranges: Vec<(f64, f64)> = g["missing"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| (r["lo"].as_f64().unwrap(), r["hi"].as_f64().unwrap()))
                .collect();
            let want: Vec<(f64, f64)> = match &v.missing {
                MissingSpec::None => vec![],
                MissingSpec::Discrete(d) => d.iter().map(|&x| (x, x)).collect(),
                MissingSpec::Range { lo, hi, discrete } => {
                    std::iter::once((*lo, *hi)).chain(discrete.map(|d| (d, d))).collect()
                }
            };
            assert_eq!(ranges, want, "{ctx}: missing");
            let vals = g["values"].as_array().unwrap();
            match ds.column(id) {
                Column::Numeric(col) => {
                    for (i, (x, w)) in col.iter().zip(vals).enumerate() {
                        assert_bits(&format!("{ctx}[{i}]"), w.as_f64().unwrap_or(f64::NAN), (!x.is_nan()).then_some(*x));
                    }
                }
                Column::Text(col) => {
                    let got: Vec<&str> = vals.iter().map(|x| x.as_str().unwrap()).collect();
                    assert_eq!(got, col.iter().map(String::as_str).collect::<Vec<_>>(), "{ctx}: values");
                }
            }
        }
    }
}

#[test]
fn truncated_files_are_format_errors() {
    for file in ["basic.sav", "uncompressed.sav", "long_string.sav", "bigendian.sav"] {
        let bytes = std::fs::read(fixture(file)).unwrap();
        let out = tmp(&format!("truncated_{file}"));
        let mut offsets: Vec<usize> = vec![0, 3, 4, 100, 175, 176, 180, bytes.len() / 2, bytes.len() - 8, bytes.len() - 1];
        offsets.extend((0..bytes.len()).step_by(7));
        // ReadStat ends compressed data with a command block holding only the end code (252).
        // Once every case has been read that block is optional, so cutting it off is not an error.
        let trailer = if bytes[bytes.len() - 8] == 252 { 8 } else { 0 };
        for cut in offsets {
            std::fs::write(&out, &bytes[..cut]).unwrap();
            match read_sav(&out) {
                Err(SavError::Format(_)) => {}
                Ok(_) if cut >= bytes.len() - trailer => {}
                other => panic!("{file} cut at {cut}/{}: expected Format error, got {other:?}", bytes.len()),
            }
        }
    }
}

/// Overwriting single bytes with extreme values must give a result or an error, never a panic.
#[test]
fn corrupted_bytes_do_not_panic() {
    for file in ["basic.sav", "long_string.sav", "bigendian.sav"] {
        let bytes = std::fs::read(fixture(file)).unwrap();
        let out = tmp(&format!("corrupt_{file}"));
        for pos in (0..bytes.len()).step_by(17) {
            for b in [0x00, 0x7f, 0xff] {
                let mut c = bytes.clone();
                c[pos] = b;
                std::fs::write(&out, &c).unwrap();
                let _ = read_sav(&out);
            }
        }
    }
}

#[test]
fn non_sav_file_is_format_error() {
    let r = read_sav(&root().join("fixtures/data/sleep.csv"));
    assert!(matches!(r, Err(SavError::Format(_))), "{r:?}");
}

#[test]
fn zsav_is_unsupported() {
    let mut bytes = std::fs::read(fixture("basic.sav")).unwrap();
    bytes[72..76].copy_from_slice(&2i32.to_le_bytes()); // compression = 2 (zlib)
    let out = tmp("fake.zsav");
    std::fs::write(&out, &bytes).unwrap();
    match read_sav(&out) {
        Err(SavError::Unsupported(m)) => assert_eq!(m, "ZSAV"),
        other => panic!("expected Unsupported(ZSAV), got {other:?}"),
    }
}

#[test]
fn missing_file_is_io_error() {
    assert!(matches!(read_sav(&fixture("does_not_exist.sav")), Err(SavError::Io(_))));
}
