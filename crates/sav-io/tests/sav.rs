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
        let ds = read_sav(&fixture(file)).unwrap_or_else(|e| panic!("{file}: {e}")).0;
        assert_matches_json(file, &ds, &exp["files"][file]);
    }
}

#[test]
fn windows_1252_text_is_decoded() {
    let ds = read_sav(&fixture("cp1252.sav")).unwrap().0;
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
        let ds = read_sav(&fixture(file)).unwrap().0;
        let skor = ds.numeric(ds.find("skor").unwrap());
        assert_eq!(skor[1].to_bits(), 0.0f64.to_bits(), "{file}");
    }
}

#[test]
fn round_trip_is_identical() {
    let exp = expected();
    for file in FIXTURES {
        let first = read_sav(&fixture(file)).unwrap().0;
        let out = tmp(&format!("roundtrip_{file}"));
        write_sav(&first, &out).unwrap_or_else(|e| panic!("{file}: {e}"));
        let second = read_sav(&out).unwrap_or_else(|e| panic!("{file} (rewritten): {e}")).0;
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
    let back = read_sav(&out).unwrap().0;
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
        let ds = read_sav(&fixture(file)).unwrap().0;
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
    for file in ["basic.sav", "uncompressed.sav", "long_string.sav", "bigendian.sav", "metadata.sav"] {
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
    for file in ["basic.sav", "long_string.sav", "bigendian.sav", "metadata.sav"] {
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

/// Minimal little-endian system file built in the test. `vars` are (short name bytes, type
/// code); string variables get their continuation records. `ext` are record 7 subtypes as
/// (subtype, size, data).
fn build_sav(vars: &[(&[u8], i32)], ext: &[(i32, i32, Vec<u8>)], compression: i32, ncases: i32, data: &[u8]) -> Vec<u8> {
    let i32b = |v: i32| v.to_le_bytes();
    let slots = |t: i32| if t == 0 { 1 } else { (t as usize).div_ceil(8) };
    let mut out = b"$FL2".to_vec();
    out.extend_from_slice(&[b' '; 60]);
    let total: usize = vars.iter().map(|v| slots(v.1)).sum();
    for v in [2, total as i32, compression, 0, ncases] {
        out.extend_from_slice(&i32b(v));
    }
    out.extend_from_slice(&100.0f64.to_le_bytes());
    out.extend_from_slice(&[b' '; 9 + 8 + 64]);
    out.extend_from_slice(&[0; 3]);
    for (name, t) in vars {
        let fmt = if *t == 0 { 0x0005_0802 } else { 0x0001_0000 | ((*t).min(255) << 8) };
        for v in [2, *t, 0, 0, fmt, fmt] {
            out.extend_from_slice(&i32b(v));
        }
        let mut n = name.to_vec();
        n.resize(8, b' ');
        out.extend_from_slice(&n);
        for _ in 1..slots(*t) {
            for v in [2, -1, 0, 0, 0, 0] {
                out.extend_from_slice(&i32b(v));
            }
            out.extend_from_slice(&[0; 8]);
        }
    }
    for (sub, size, d) in ext {
        for v in [7, *sub, *size, d.len() as i32 / size] {
            out.extend_from_slice(&i32b(v));
        }
        out.extend_from_slice(d);
    }
    out.extend_from_slice(&i32b(999));
    out.extend_from_slice(&i32b(0));
    out.extend_from_slice(data);
    out
}

fn integer_info(char_code: i32) -> (i32, i32, Vec<u8>) {
    (3, 4, [20, 0, 0, -1, 1, 0, 2, char_code].iter().flat_map(|v: &i32| v.to_le_bytes()).collect())
}

/// A header claiming 2^31-1 cases must not make the reader reserve memory per claimed case
/// (it used to reserve min(ncases, file size) values for every variable and abort).
#[test]
fn huge_case_count_with_many_variables_is_an_error_not_an_abort() {
    let names: Vec<Vec<u8>> = (0..20_000).map(|i| format!("V{i:07}").into_bytes()).collect();
    let vars: Vec<(&[u8], i32)> = names.iter().map(|n| (n.as_slice(), 0)).collect();
    for compression in [0, 1] {
        let bytes = build_sav(&vars, &[], compression, i32::MAX, &vec![0u8; 2_000_000]);
        let out = tmp(&format!("huge_ncases_{compression}.sav"));
        std::fs::write(&out, bytes).unwrap();
        let r = read_sav(&out);
        assert!(matches!(r, Err(SavError::Format(_))), "compression {compression}: {:?}", r.map(|d| d.0.n_cases()));
    }
}

/// Old files have 2 integers (measure, alignment) per variable in subtype 11; with 3
/// variables the count (6) is a multiple of 3 and must still be read as 2 per variable.
#[test]
fn display_record_with_two_values_per_variable() {
    let vars: [(&[u8], i32); 3] = [(b"A", 0), (b"B", 0), (b"C", 0)];
    let display: Vec<u8> = [1, 1, 2, 1, 1, 1].iter().flat_map(|v: &i32| v.to_le_bytes()).collect();
    let data: Vec<u8> = [1.0f64, 2.0, 3.0].iter().flat_map(|v| v.to_le_bytes()).collect();
    let out = tmp("display2.sav");
    std::fs::write(&out, build_sav(&vars, &[(11, 4, display)], 0, 1, &data)).unwrap();
    let ds = read_sav(&out).unwrap().0;
    let m: Vec<Measure> = ids(&ds).into_iter().map(|id| ds.var(id).measure).collect();
    assert_eq!(m, [Measure::Nominal, Measure::Ordinal, Measure::Nominal]);
}

/// Code page 932 (Shift_JIS) from subtype 3 decodes names and text.
#[test]
fn shift_jis_code_page() {
    let (name, _, _) = encoding_rs::SHIFT_JIS.encode("名前");
    let mut data = encoding_rs::SHIFT_JIS.encode("日本").0.into_owned();
    data.resize(8, b' ');
    let out = tmp("sjis.sav");
    std::fs::write(&out, build_sav(&[(&name, 8)], &[integer_info(932)], 0, 1, &data)).unwrap();
    let ds = read_sav(&out).unwrap().0;
    assert_eq!(ds.var(VarId(0)).name, "名前");
    match ds.column(VarId(0)) {
        Column::Text(v) => assert_eq!(v[0], "日本"),
        Column::Numeric(_) => panic!("expected a string"),
    }
}

/// The very long string record (subtype 14) is in the file encoding, like the names.
#[test]
fn very_long_string_record_uses_file_encoding() {
    let vars: [(&[u8], i32); 2] = [(b"K\xd6M", 255), (b"K\xd6M1", 48)];
    let ext = [integer_info(1252), (14, 1, b"K\xd6M=00300\0\t".to_vec())];
    let mut data = vec![b'a'; 255];
    data.push(b' ');
    data.extend_from_slice(&[b'b'; 45]);
    data.resize(256 + 48, b' ');
    let out = tmp("vls_cp1252.sav");
    std::fs::write(&out, build_sav(&vars, &ext, 0, 1, &data)).unwrap();
    let ds = read_sav(&out).unwrap().0;
    assert_eq!(ds.n_vars(), 1);
    assert_eq!(ds.var(VarId(0)).name, "KÖM");
    assert_eq!(ds.var(VarId(0)).var_type, VarType::String { width: 300 });
    match ds.column(VarId(0)) {
        Column::Text(v) => assert_eq!(v[0], "a".repeat(255) + &"b".repeat(45)),
        Column::Numeric(_) => panic!("expected a string"),
    }
}

#[test]
fn reference_fixtures_read_without_notes() {
    for file in FIXTURES {
        let (_, report) = read_sav(&fixture(file)).unwrap();
        assert!(report.notes.is_empty(), "{file}: {:?}", report.notes);
    }
}

/// 2026-01-01 as an SPSS date: seconds since 1582-10-14.
const JAN_1_2026: f64 = 13_986_604_800.0;

fn strings(xs: &[&str]) -> Vec<String> {
    xs.iter().map(|s| s.to_string()).collect()
}

fn str_labels(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

/// Everything metadata.sav holds beyond sav_basic (see gen_refs.py write_sav_metadata_fixture).
fn assert_metadata(ctx: &str, ds: &Dataset) {
    let id = |n: &str| ds.find(n).unwrap_or_else(|| panic!("{ctx}: no {n}"));
    assert_eq!(ds.weight(), Some(id("bobot")), "{ctx}: weight");
    assert_eq!(ds.file_label(), Some("Survei kepuasan"), "{ctx}");
    assert_eq!(ds.documents(), ["Dokumen baris satu", "baris dua"], "{ctx}");
    let tgl = ds.var(id("tgl"));
    assert_eq!((tgl.format_type, tgl.width, tgl.decimals), (Some(20), 11, 0), "{ctx}: DATE11");
    assert!(tgl.has_date_format());
    assert_eq!(tgl.label.as_deref(), Some("Tanggal survei"));
    assert_eq!(ds.numeric(id("tgl"))[0], JAN_1_2026, "{ctx}");
    assert_eq!(ds.numeric(id("tgl"))[1], JAN_1_2026 + 86_400.0, "{ctx}");
    assert!(ds.numeric(id("tgl"))[2].is_nan(), "{ctx}");
    let harga = ds.var(id("harga"));
    assert_eq!((harga.format_type, harga.width, harga.decimals), (Some(4), 10, 2), "{ctx}: DOLLAR10.2");
    assert_eq!(ds.var(id("id")).format_type, None, "{ctx}: F is None");
    let kode = ds.var(id("kode"));
    assert_eq!(kode.var_type, VarType::String { width: 1 }, "{ctx}");
    assert_eq!(kode.string_value_labels, str_labels(&[("L", "Laki-laki"), ("P", "Perempuan")]), "{ctx}");
    assert_eq!(kode.string_missing, strings(&["X"]), "{ctx}");
    let kota = ds.var(id("kota"));
    assert_eq!(kota.var_type, VarType::String { width: 15 }, "{ctx}");
    assert_eq!(kota.string_value_labels, str_labels(&[("Bandung", "Kota Bandung")]), "{ctx}");
    assert_eq!(kota.string_missing, strings(&["x"]), "{ctx}");
    match ds.column(id("kota")) {
        Column::Text(t) => assert_eq!(t, &strings(&["Jakarta Selatan", "Bandung", "x"]), "{ctx}"),
        Column::Numeric(_) => panic!("{ctx}: kota should be a string"),
    }
}

#[test]
fn metadata_fixture_is_read_with_notes() {
    let (ds, report) = read_sav(&fixture("metadata.sav")).unwrap();
    assert_metadata("metadata.sav", &ds);
    assert_eq!(
        report.notes,
        [
            "Weight variable bobot is ignored by Statik analyses (results are unweighted).",
            "Variable tgl has a date format; dates are shown as numbers (seconds since 1582-10-14).",
        ]
    );
}

/// Weight, formats, string value labels / missing values, file label and documents survive a
/// save, and an independent reader (pyreadstat) sees them in Statik's file.
#[test]
fn metadata_fixture_round_trips() {
    let (first, _) = read_sav(&fixture("metadata.sav")).unwrap();
    let out = tmp("roundtrip_metadata.sav");
    write_sav(&first, &out).unwrap();
    let (second, report) = read_sav(&out).unwrap();
    assert_metadata("rewritten", &second);
    assert_eq!(report.notes.len(), 2, "{:?}", report.notes);

    let Some(got) = pyreadstat_json(&out) else { return };
    assert_eq!(got["file_label"], "Survei kepuasan");
    assert_eq!(got["notes"], serde_json::json!(["Dokumen baris satu", "baris dua"]));
    assert_eq!(got["weight_index"], 2);
    let vars = got["variables"].as_array().unwrap();
    let by_name = |n: &str| vars.iter().find(|v| v["name"] == n).unwrap_or_else(|| panic!("{n}"));
    assert_eq!(by_name("tgl")["format"], "DATE11");
    assert_eq!(by_name("tgl")["values"][0], JAN_1_2026);
    assert_eq!(by_name("harga")["format"], "DOLLAR10.2");
    assert_eq!(by_name("id")["format"], "F8.0");
    assert_eq!(by_name("kode")["value_labels"], serde_json::json!([["L", "Laki-laki"], ["P", "Perempuan"]]));
    assert_eq!(by_name("kode")["missing"], serde_json::json!([{"lo": "X", "hi": "X"}]));
    assert_eq!(by_name("kota")["format"], "A15");
    assert_eq!(by_name("kota")["value_labels"], serde_json::json!([["Bandung", "Kota Bandung"]]));
    assert_eq!(by_name("kota")["missing"], serde_json::json!([{"lo": "x", "hi": "x"}]));
}

/// Reserved words are not variable names: a file using one is renamed on read, with a note.
#[test]
fn reserved_names_are_renamed_on_read_with_a_note() {
    let vars: [(&[u8], i32); 2] = [(b"AND", 0), (b"X", 0)];
    let data: Vec<u8> = [1.0f64, 2.0].iter().flat_map(|v| v.to_le_bytes()).collect();
    let out = tmp("reserved.sav");
    std::fs::write(&out, build_sav(&vars, &[], 0, 1, &data)).unwrap();
    let (ds, report) = read_sav(&out).unwrap();
    assert_eq!(ds.var(VarId(0)).name, "AND_1");
    assert_eq!(report.notes, ["Variable name 'AND' is not valid in Statik; it was renamed to 'AND_1'."]);
}

/// A numeric column on a string variable is an internal inconsistency, not an invalid file;
/// the failed save leaves the existing file untouched and no temporary file behind.
/// Number of `<file>.<something>.tmp` temporary files next to `target`.
fn leftover_temps(target: &std::path::Path) -> usize {
    let prefix = format!("{}.", target.file_name().unwrap().to_string_lossy());
    std::fs::read_dir(target.parent().unwrap())
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with(&prefix) && n.ends_with(".tmp") && n.len() > prefix.len() + 4)
        .count()
}

#[test]
fn users_own_dot_tmp_file_survives_a_save() {
    let out = tmp("user_tmp.sav");
    let user_tmp = tmp("user_tmp.sav.tmp");
    std::fs::write(&user_tmp, b"not statik's").unwrap();
    let (ds, _) = read_sav(&fixture("basic.sav")).unwrap();
    write_sav(&ds, &out).unwrap();
    assert_eq!(std::fs::read(&user_tmp).unwrap(), b"not statik's");
    assert_eq!(read_sav(&out).unwrap().0.n_vars(), ds.n_vars());
    assert_eq!(leftover_temps(&out), 0);
}

#[test]
fn failed_save_is_internal_error_and_keeps_existing_file() {
    let out = tmp("keep_existing.sav");
    std::fs::write(&out, b"original bytes").unwrap();
    let mut ds = Dataset::new();
    ds.add_variable(Variable::numeric("x"), Column::Numeric(vec![1.0])).unwrap();
    ds.var_mut(VarId(0)).var_type = VarType::String { width: 8 };
    let e = write_sav(&ds, &out).unwrap_err();
    assert!(matches!(e, SavError::Internal(_)), "{e:?}");
    assert_eq!(e.to_string(), "Cannot save: variable 'x' has a numeric column but a string type");
    assert_eq!(std::fs::read(&out).unwrap(), b"original bytes");
    assert_eq!(leftover_temps(&out), 0);
}

#[test]
fn save_replaces_existing_file_without_leaving_a_temporary_file() {
    let out = tmp("replace_existing.sav");
    std::fs::write(&out, b"old").unwrap();
    let (ds, _) = read_sav(&fixture("basic.sav")).unwrap();
    write_sav(&ds, &out).unwrap();
    assert_eq!(read_sav(&out).unwrap().0.n_vars(), ds.n_vars());
    assert_eq!(leftover_temps(&out), 0);
}

/// A save into a folder that does not exist fails with an i/o error and creates nothing.
#[test]
fn save_into_missing_folder_is_io_error() {
    let out = tmp("no_such_dir").join("x.sav");
    let (ds, _) = read_sav(&fixture("basic.sav")).unwrap();
    assert!(matches!(write_sav(&ds, &out), Err(SavError::Io(_))));
    assert!(!out.exists());
}

/// String missing values: at most 3, each at most 8 bytes (the .sav format's limits).
#[test]
fn string_missing_value_limits() {
    let mut ds = Dataset::new();
    let mut v = Variable::string("s", 20);
    v.string_missing = strings(&["123456789"]);
    ds.add_variable(v, Column::Text(strings(&["a"]))).unwrap();
    assert!(matches!(write_sav(&ds, &tmp("smiss1.sav")), Err(SavError::Unsupported(_))));
    ds.var_mut(VarId(0)).string_missing = strings(&["a", "b", "c", "d"]);
    assert!(matches!(write_sav(&ds, &tmp("smiss2.sav")), Err(SavError::Unsupported(_))));
    ds.var_mut(VarId(0)).string_missing = strings(&["a", "b", "c"]);
    write_sav(&ds, &tmp("smiss3.sav")).unwrap();
    let (back, _) = read_sav(&tmp("smiss3.sav")).unwrap();
    assert_eq!(back.var(VarId(0)).string_missing, strings(&["a", "b", "c"]));
}
