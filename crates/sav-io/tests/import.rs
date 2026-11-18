use dataset::{Column, Dataset, VarType};
use sav_io::{CsvOptions, SavError, import_csv, import_xlsx, open_any, sanitize_name};
use std::path::PathBuf;

fn fixture(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures").join(rel)
}

fn temp_csv(name: &str, content: &[u8]) -> PathBuf {
    let dir = std::env::temp_dir().join("statik_sav_io_import_tests");
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(name);
    std::fs::write(&p, content).unwrap();
    p
}

fn auto() -> CsvOptions {
    CsvOptions { delimiter: None, decimal_comma: None, header: true }
}

fn names(ds: &Dataset) -> Vec<String> {
    (0..ds.n_vars()).map(|i| ds.var(dataset::VarId(i)).name.clone()).collect()
}

fn num(ds: &Dataset, name: &str) -> Vec<f64> {
    ds.numeric(ds.find(name).unwrap()).to_vec()
}

#[test]
fn indonesian_semicolon_csv() {
    let ds = import_csv(&fixture("data/indo_semicolon.csv"), &auto()).unwrap();
    assert_eq!(names(&ds), ["Jenis_Kelamin", "Nilai_Ujian", "Umur"]);
    for i in 0..3 {
        assert!(matches!(ds.var(dataset::VarId(i)).var_type, VarType::Numeric));
    }
    assert_eq!(ds.n_cases(), 5);
    let n = num(&ds, "Nilai_Ujian");
    assert_eq!(n[0], 78.5);
    assert_eq!(n[1], 82.25);
    assert_eq!(n[2], 90.0);
    assert!(n[4].is_nan(), "empty cell is NaN");
    assert_eq!(ds.var(ds.find("Nilai_Ujian").unwrap()).decimals, 2);
    assert_eq!(ds.var(ds.find("Umur").unwrap()).decimals, 0);
    assert_eq!(ds.var(ds.find("Jenis_Kelamin").unwrap()).label.as_deref(), Some("Jenis Kelamin"));
}

#[test]
fn messy_headers_get_valid_unique_names() {
    let ds = import_csv(&fixture("data/messy_headers.csv"), &auto()).unwrap();
    let want = ["VAR00001", "Nama", "Nama_1", "V1abc", &"a".repeat(64)];
    assert_eq!(names(&ds), want);
    for n in names(&ds) {
        dataset::validate_name(&n).unwrap();
    }
    // Second data column is text, the last one numeric with 1 decimal.
    assert!(matches!(ds.var(dataset::VarId(1)).var_type, VarType::String { width: 4 }));
    assert_eq!(ds.var(dataset::VarId(4)).decimals, 1);
}

#[test]
fn ragged_row_reports_line_number() {
    let p = temp_csv("ragged.csv", b"a,b,c\n1,2,3\n4,5\n6,7,8\n");
    match import_csv(&p, &auto()) {
        Err(SavError::Csv(m)) => assert!(m.starts_with("baris 3:"), "{m}"),
        other => panic!("expected Format error, got {other:?}"),
    }
}

#[test]
fn many_decimals_are_capped_at_four() {
    let p = temp_csv("dec.csv", b"x\n3.66666666666667\n1.5\n");
    let ds = import_csv(&p, &auto()).unwrap();
    let v = ds.var(ds.find("x").unwrap());
    assert_eq!(v.decimals, 4);
    assert_eq!(v.width, 8);
    assert_eq!(num(&ds, "x")[0], 3.66666666666667);
}

#[test]
fn long_numbers_widen_the_column() {
    let p = temp_csv("wide.csv", b"x\n123456789012\n1\n");
    let ds = import_csv(&p, &auto()).unwrap();
    assert_eq!(ds.var(ds.find("x").unwrap()).width, 13);
}

#[test]
fn decimal_comma_thousands_separator_rules() {
    let p = temp_csv("thou.csv", b"a;b\n1.234,5;1.23,4\n2;x\n");
    let ds = import_csv(&p, &auto()).unwrap();
    // Column a: "1.234,5" is 1234.5 and "2" is 2.
    assert_eq!(num(&ds, "a"), [1234.5, 2.0]);
    // Column b: "1.23,4" is not a valid grouping, so the column is text.
    assert!(matches!(ds.var(ds.find("b").unwrap()).var_type, VarType::String { width: 6 }));
}

#[test]
fn comma_delimiter_does_not_accept_comma_decimals() {
    let p = temp_csv("comma.csv", b"a,b\n1,2\n3,4\n");
    let ds = import_csv(&p, &auto()).unwrap();
    assert_eq!(ds.n_vars(), 2);
    assert_eq!(num(&ds, "a"), [1.0, 3.0]);
    // Explicit delimiter ';' with plain decimal points.
    let p = temp_csv("semi_dot.csv", b"a;b\n1.5;2\n3.25;4\n");
    let opt = CsvOptions { delimiter: Some(b';'), decimal_comma: Some(false), header: true };
    let ds = import_csv(&p, &opt).unwrap();
    assert_eq!(num(&ds, "a"), [1.5, 3.25]);
}

#[test]
fn quoted_fields_and_text_columns() {
    let p = temp_csv("quoted.csv", "nama,ket\n\"Budi, S.\",\"a \"\"b\"\"\"\nAni,Café  \n".as_bytes());
    let ds = import_csv(&p, &auto()).unwrap();
    match ds.column(ds.find("nama").unwrap()) {
        Column::Text(t) => assert_eq!(t, &["Budi, S.", "Ani"]),
        _ => panic!("text expected"),
    }
    match ds.column(ds.find("ket").unwrap()) {
        Column::Text(t) => assert_eq!(t, &["a \"b\"", "Café"]),
        _ => panic!("text expected"),
    }
    assert!(matches!(ds.var(ds.find("ket").unwrap()).var_type, VarType::String { width: 5 }));
}

#[test]
fn no_header_option_names_variables() {
    let p = temp_csv("nohdr.csv", b"1,2\n3,4\n");
    let ds = import_csv(&p, &CsvOptions { header: false, ..auto() }).unwrap();
    assert_eq!(names(&ds), ["VAR00001", "VAR00002"]);
    assert_eq!(ds.n_cases(), 2);
}

#[test]
fn negative_zero_is_normalised() {
    let p = temp_csv("negzero.csv", b"x\n-0\n1.5\n");
    let ds = import_csv(&p, &auto()).unwrap();
    assert!(num(&ds, "x")[0].is_sign_positive());
}

#[test]
fn sanitize_name_rules() {
    let none = |_: &str| false;
    assert_eq!(sanitize_name("Jenis Kelamin", &none), "Jenis_Kelamin");
    assert_eq!(sanitize_name("1abc", &none), "V1abc");
    assert_eq!(sanitize_name("_x", &none), "V_x");
    assert_eq!(sanitize_name("", &none), "VAR00001");
    assert_eq!(sanitize_name("   ", &none), "VAR00001");
    assert_eq!(sanitize_name(&"b".repeat(70), &none), "b".repeat(64));
    assert_eq!(sanitize_name("a-b/c", &none), "a_b_c");
    assert_eq!(sanitize_name("Café", &none), "Café");
    let taken = |s: &str| s == "x" || s == "x_1" || s == "VAR00001";
    assert_eq!(sanitize_name("x", &taken), "x_2");
    assert_eq!(sanitize_name("", &taken), "VAR00002");
    // Suffix still fits in 64 characters.
    let long = "c".repeat(64);
    let t = |s: &str| s == long;
    let got = sanitize_name(&long, &t);
    assert_eq!(got.chars().count(), 64);
    assert!(got.ends_with("_1"));
}

#[test]
fn xlsx_second_sheet_with_empty_cells() {
    let ds = import_xlsx(&fixture("data/sample.xlsx"), Some("Kosong"), true).unwrap();
    assert_eq!(names(&ds), ["a", "b", "c"]);
    let a = num(&ds, "a");
    assert_eq!(a[0], 1.0);
    assert!(a[1].is_nan());
    assert_eq!(a[2], 3.0);
    match ds.column(ds.find("b").unwrap()) {
        Column::Text(t) => assert_eq!(t, &["x", "y", ""]),
        _ => panic!("text expected"),
    }
    assert_eq!(num(&ds, "c"), [10.5, 20.0, 30.25]);
    assert_eq!(ds.var(ds.find("c").unwrap()).decimals, 2);
}

#[test]
fn xlsx_first_sheet_default_and_dates_as_serials() {
    let ds = import_xlsx(&fixture("data/sample.xlsx"), None, true).unwrap();
    assert_eq!(names(&ds), ["Jenis_Kelamin", "Nama", "Nilai", "Tanggal"]);
    assert_eq!(num(&ds, "Jenis_Kelamin"), [1.0, 2.0, 1.0, 2.0]);
    // 2026-01-01 is Excel serial 46023.
    assert_eq!(num(&ds, "Tanggal")[0], 46023.0);
    assert_eq!(num(&ds, "Nilai")[1], 82.25);
    assert!(matches!(ds.var(ds.find("Nama").unwrap()).var_type, VarType::String { width: 5 }));
}

#[test]
fn xlsx_unknown_sheet_is_an_error() {
    assert!(matches!(import_xlsx(&fixture("data/sample.xlsx"), Some("Nope"), true), Err(SavError::Xlsx(_))));
}

#[test]
fn open_any_dispatches_on_extension() {
    let sav = open_any(&fixture("sav/basic.sav")).unwrap();
    assert!(sav.find("gender").is_some());
    let csv = open_any(&fixture("data/indo_semicolon.csv")).unwrap();
    assert!(csv.find("Nilai_Ujian").is_some());
    let xlsx = open_any(&fixture("data/sample.xlsx")).unwrap();
    assert!(xlsx.find("Nilai").is_some());
    let p = temp_csv("x.docx", b"zzz");
    assert!(matches!(open_any(&p), Err(SavError::UnknownType(_))));
}

#[test]
fn single_column_with_decimal_commas() {
    let p = temp_csv("single.csv", b"Nilai\r\n78,5\r\n80\r\n");
    let ds = import_csv(&p, &auto()).unwrap();
    assert_eq!(names(&ds), ["Nilai"]);
    assert_eq!(num(&ds, "Nilai"), [78.5, 80.0]);
    assert_eq!(ds.var(ds.find("Nilai").unwrap()).decimals, 1);
}

#[test]
fn errors_name_their_source() {
    let p = temp_csv("ragged2.csv", b"a,b\n1,2\n3\n");
    let e = import_csv(&p, &auto()).unwrap_err().to_string();
    assert!(e.starts_with("CSV tidak valid: baris 3"), "{e}");
    let e = import_xlsx(&fixture("data/sample.xlsx"), Some("Nope"), true).unwrap_err().to_string();
    assert!(e.starts_with("XLSX tidak valid:"), "{e}");
}

#[test]
fn integers_beyond_15_digits_stay_text_but_leading_zeros_stay_numeric() {
    let p = temp_csv("nik.csv", b"nik,kode\n3201234567890123,007\n3201234567890124,008\n");
    let ds = import_csv(&p, &auto()).unwrap();
    match ds.column(ds.find("nik").unwrap()) {
        Column::Text(t) => assert_eq!(t[0], "3201234567890123"),
        _ => panic!("16-digit column must be text"),
    }
    assert_eq!(num(&ds, "kode"), [7.0, 8.0]);
    // Exactly 15 digits is still numeric.
    let p = temp_csv("d15.csv", b"x\n123456789012345\n");
    let ds = import_csv(&p, &auto()).unwrap();
    assert_eq!(num(&ds, "x"), [123456789012345.0]);
}
