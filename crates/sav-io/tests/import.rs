use dataset::{Column, Dataset, VarId, VarType};
use sav_io::{
    CsvOptions, SNIFF_BYTES, SavError, csv_prefix, import_csv, import_xlsx, open_any, sanitize_name, sniff_csv,
    sniff_csv_text, xlsx_sheet_names,
};
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
        Err(SavError::Csv(m)) => assert_eq!(m, "line 3 has 2 fields, expected 3"),
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
    let (sav, _) = open_any(&fixture("sav/basic.sav")).unwrap();
    assert!(sav.find("gender").is_some());
    let (csv, report) = open_any(&fixture("data/indo_semicolon.csv")).unwrap();
    assert!(report.notes.is_empty());
    assert!(csv.find("Nilai_Ujian").is_some());
    let (xlsx, _) = open_any(&fixture("data/sample.xlsx")).unwrap();
    assert!(xlsx.find("Nilai").is_some());
    let p = temp_csv("x.docx", b"zzz");
    let e = open_any(&p).unwrap_err();
    assert!(matches!(e, SavError::UnknownType(_)));
    assert_eq!(e.to_string(), "Unsupported file type: .docx");
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
    assert_eq!(e, "Invalid CSV: line 3 has 1 field, expected 2");
    let e = import_xlsx(&fixture("data/sample.xlsx"), Some("Nope"), true).unwrap_err().to_string();
    assert_eq!(e, "Invalid XLSX: sheet 'Nope' not found");
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

/// Indonesian Rupiah amounts use `.` for thousands. In a `;` file with no cell that only makes
/// sense with a decimal point, `750.000` is 750000 (it used to import as 750).
#[test]
fn semicolon_file_reads_dot_thousands_as_thousands() {
    let p = temp_csv("rupiah.csv", b"nama;gaji;umur\nAni;750.000;21\nBudi;500.000;22\nCici;1.250;23\n");
    let ds = import_csv(&p, &auto()).unwrap();
    assert_eq!(num(&ds, "gaji"), [750000.0, 500000.0, 1250.0]);
    assert_eq!(ds.var(ds.find("gaji").unwrap()).decimals, 0);
    assert_eq!(num(&ds, "umur"), [21.0, 22.0, 23.0]);
    // Tab-separated works the same way.
    let p = temp_csv("rupiah.tsv", b"gaji\tumur\n1.500.000\t21\n2.000\t22\n");
    let ds = import_csv(&p, &auto()).unwrap();
    assert_eq!(num(&ds, "gaji"), [1500000.0, 2000.0]);
}

/// Thousands dots and decimal commas together (the usual Indonesian Excel export).
#[test]
fn semicolon_file_mixing_thousands_dots_and_decimal_commas() {
    let p = temp_csv("mixed.csv", b"gaji;nilai\n750.000;78,5\n1.250;80\n");
    let ds = import_csv(&p, &auto()).unwrap();
    assert_eq!(num(&ds, "gaji"), [750000.0, 1250.0]);
    assert_eq!(num(&ds, "nilai"), [78.5, 80.0]);
}

/// A cell that is only valid with a decimal point (`78.5`) switches the file to decimal points,
/// so `1.250` in that file is 1.25.
#[test]
fn semicolon_file_with_dot_decimal_evidence_uses_decimal_points() {
    let p = temp_csv("dotsemi.csv", b"a;b\n78.5;1.250\n2;3\n");
    let ds = import_csv(&p, &auto()).unwrap();
    assert_eq!(num(&ds, "a"), [78.5, 2.0]);
    assert_eq!(num(&ds, "b"), [1.25, 3.0]);
    assert_eq!(ds.var(ds.find("b").unwrap()).decimals, 3);
}

/// With a `,` delimiter a `.` is always the decimal point.
#[test]
fn comma_file_keeps_dot_decimals() {
    let p = temp_csv("commadot.csv", b"x,y\n750.000,1\n");
    let ds = import_csv(&p, &auto()).unwrap();
    assert_eq!(num(&ds, "x"), [750.0]);
}

#[test]
fn sniff_csv_reports_detection_and_first_rows() {
    let pv = sniff_csv(&fixture("data/indo_semicolon.csv"), &auto(), 2).unwrap();
    assert_eq!(pv.delimiter, b';');
    assert!(pv.decimal_comma);
    assert_eq!(pv.header, ["Jenis_Kelamin", "Nilai_Ujian", "Umur"]);
    assert_eq!(pv.raw_header, ["Jenis Kelamin", "Nilai Ujian", "Umur"]);
    assert_eq!(pv.rows, [vec!["1", "78,5", "20"], vec!["2", "82,25", "21"]]);

    let p = temp_csv("sniff_rupiah.csv", b"gaji;umur\n750.000;21\n");
    let pv = sniff_csv(&p, &auto(), 10).unwrap();
    assert!(pv.decimal_comma);
    assert_eq!(pv.rows.len(), 1);

    let p = temp_csv("sniff_comma.csv", b"a,b\n1.5,2\n");
    let pv = sniff_csv(&p, &CsvOptions { header: false, ..auto() }, 10).unwrap();
    assert_eq!(pv.delimiter, b',');
    assert!(!pv.decimal_comma);
    assert_eq!(pv.header, ["VAR00001", "VAR00002"]);
    assert_eq!(pv.raw_header, ["", ""]);
    assert_eq!(pv.rows, [vec!["a", "b"], vec!["1.5", "2"]]);

    // Explicit options are reported back as given.
    let pv = sniff_csv(&p, &CsvOptions { delimiter: Some(b';'), decimal_comma: Some(true), header: true }, 10).unwrap();
    assert_eq!(pv.delimiter, b';');
    assert!(pv.decimal_comma);
    assert_eq!(pv.raw_header, ["a,b"]);
}

#[test]
fn sniff_csv_reads_only_a_bounded_prefix() {
    // ~3 MB of valid rows (with a quoted field holding a line break and a multi-byte character),
    // then a malformed tail: the preview reads only the first SNIFF_BYTES and succeeds; the full
    // import still checks the whole file.
    let mut text = String::from("nama;nilai\n");
    let mut i = 0;
    while text.len() < 3 * SNIFF_BYTES {
        text.push_str(&format!("\"Siswa é\n{i}\";{i},5\n"));
        i += 1;
    }
    text.push_str("too;many;fields\n");
    let p = temp_csv("sniff_big.csv", text.as_bytes());
    let pv = sniff_csv(&p, &auto(), 10).unwrap();
    assert_eq!(pv.delimiter, b';');
    assert!(pv.decimal_comma);
    assert_eq!(pv.rows.len(), 10);
    assert_eq!(pv.rows[0], ["Siswa é\n0", "0,5"]);
    assert!(import_csv(&p, &auto()).is_err());

    // The prefix ends with a complete record, never inside a quoted field or a character.
    let pre = csv_prefix(&p, SNIFF_BYTES).unwrap();
    assert!(pre.len() <= SNIFF_BYTES);
    assert!(pre.ends_with(",5\n"), "{:?}", &pre[pre.len() - 20..]);
    for cut in [SNIFF_BYTES - 1, SNIFF_BYTES - 7, 1000, 1001, 1002, 1003] {
        let pre = csv_prefix(&p, cut).unwrap();
        assert!(pre.ends_with(",5\n"), "cut {cut}");
        let pv = sniff_csv_text(&pre, &auto(), usize::MAX).unwrap();
        assert!(pv.rows.iter().all(|r| r.len() == 2));
    }
    // A small file is read whole.
    let small = csv_prefix(&fixture("data/indo_semicolon.csv"), SNIFF_BYTES).unwrap();
    assert!(small.trim_end().ends_with("1;;20"), "{small:?}");
}

#[test]
fn xlsx_sheet_names_lists_sheets_in_order() {
    assert_eq!(xlsx_sheet_names(&fixture("data/sample.xlsx")).unwrap(), ["Data", "Kosong"]);
    assert!(matches!(xlsx_sheet_names(&fixture("data/nope.xlsx")), Err(SavError::Io(_))));
    let p = temp_csv("notxlsx.xlsx", b"zzz");
    assert!(matches!(xlsx_sheet_names(&p), Err(SavError::Xlsx(_))));
}

#[test]
fn import_errors_are_english() {
    let p = temp_csv("empty.csv", b"");
    assert_eq!(import_csv(&p, &auto()).unwrap_err().to_string(), "Invalid CSV: the file is empty");
    let p = temp_csv("unclosed.csv", b"a,b\n1,\"x\n");
    let comma = CsvOptions { delimiter: Some(b','), ..auto() };
    assert_eq!(import_csv(&p, &comma).unwrap_err().to_string(), "Invalid CSV: line 2: a quoted field is not closed");
    let p = temp_csv("notxlsx2.xlsx", b"zzz");
    let e = import_xlsx(&p, None, true).unwrap_err().to_string();
    assert!(e.starts_with("Invalid XLSX: cannot open "), "{e}");
}

/// Reserved words (AND, TO, BY, ...) are not variable names: imports add `_1`.
#[test]
fn reserved_word_headers_get_a_suffix() {
    let p = temp_csv("reserved.csv", b"and,TO,by,x\n1,2,3,4\n");
    let ds = import_csv(&p, &auto()).unwrap();
    assert_eq!(names(&ds), ["and_1", "TO_1", "by_1", "x"]);
    assert_eq!(ds.var(VarId(0)).label.as_deref(), Some("and"));
    let none = |_: &str| false;
    assert_eq!(sanitize_name("With", &none), "With_1");
    let taken = |s: &str| s == "ALL_1";
    assert_eq!(sanitize_name("ALL", &taken), "ALL_2");
}

/// The 64-name limit is in UTF-8 bytes; names are cut on a character boundary.
#[test]
fn sanitize_name_limits_bytes_not_characters() {
    let none = |_: &str| false;
    let got = sanitize_name(&"é".repeat(40), &none);
    assert_eq!(got, "é".repeat(32));
    let got = sanitize_name(&format!("a{}", "é".repeat(40)), &none);
    assert_eq!(got, format!("a{}", "é".repeat(31))); // 63 bytes: the next 'é' would not fit
    let full = "é".repeat(32);
    let t = |s: &str| s == full;
    let got = sanitize_name(&full, &t);
    assert_eq!(got, format!("{}_1", "é".repeat(31)));
    assert!(got.len() <= 64);
    dataset::validate_name(&got).unwrap();
}
