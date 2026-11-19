use dataset::*;

fn ds() -> Dataset {
    let mut d = Dataset::new();
    d.add_variable(Variable::numeric("a"), Column::Numeric(vec![1.0, 2.0, 3.0])).unwrap();
    d.add_variable(Variable::string("s", 8), Column::Text(vec!["x".into(), "y".into(), "z".into()])).unwrap();
    d
}

#[test]
fn insert_and_delete_variable() {
    let mut d = ds();
    let id = d.insert_variable(1, Variable::numeric("b"), Column::Numeric(vec![9.0; 3])).unwrap();
    assert_eq!(id, VarId(1));
    assert_eq!(d.var(VarId(2)).name, "s");
    assert_eq!(d.numeric(VarId(1)), &[9.0; 3]);
    assert!(d.insert_variable(1, Variable::numeric("B"), Column::Numeric(vec![0.0; 3])).is_err());
    assert!(d.insert_variable(9, Variable::numeric("c"), Column::Numeric(vec![0.0; 3])).is_err());
    assert!(d.insert_variable(0, Variable::numeric("c"), Column::Numeric(vec![0.0; 2])).is_err());
    assert_eq!(d.n_vars(), 3);
    d.delete_variable(VarId(0)).unwrap();
    assert_eq!(d.var(VarId(0)).name, "b");
    assert!(d.delete_variable(VarId(5)).is_err());
    assert!(d.try_var(VarId(2)).is_none());
    assert!(d.try_var(VarId(1)).is_some());
}

#[test]
fn rename() {
    let mut d = ds();
    assert!(d.rename_variable(VarId(0), "s").is_err());
    assert!(d.rename_variable(VarId(0), "1bad").is_err());
    assert!(d.rename_variable(VarId(9), "q").is_err());
    d.rename_variable(VarId(0), "A").unwrap(); // case change of itself is fine
    d.rename_variable(VarId(0), "q").unwrap();
    assert_eq!(d.var(VarId(0)).name, "q");
}

#[test]
fn cases() {
    let mut d = ds();
    d.insert_case(1);
    assert_eq!(d.n_cases(), 4);
    assert!(d.numeric(VarId(0))[1].is_nan());
    assert_eq!(d.numeric(VarId(0))[2], 2.0);
    let Column::Text(t) = d.column(VarId(1)) else { panic!() };
    assert_eq!(t, &["x", "", "y", "z"]);
    d.delete_case(1).unwrap();
    assert_eq!(d.numeric(VarId(0)), &[1.0, 2.0, 3.0]);
    assert!(d.delete_case(3).is_err());
}

#[test]
fn cell_edits() {
    let mut d = ds();
    d.set_numeric(VarId(0), 0, -0.0).unwrap();
    assert!(d.numeric(VarId(0))[0].is_sign_positive());
    assert!(d.set_numeric(VarId(1), 0, 1.0).is_err());
    assert!(d.set_numeric(VarId(0), 7, 1.0).is_err());
    d.set_text(VarId(1), 2, "w").unwrap();
    assert!(d.set_text(VarId(0), 0, "w").is_err());
    assert!(d.set_text(VarId(1), 9, "w").is_err());
}

#[test]
fn type_conversion() {
    let mut d = Dataset::new();
    d.add_variable(Variable::numeric("n"), Column::Numeric(vec![1.5, f64::NAN, 1234567.891])).unwrap();
    d.set_type(VarId(0), VarType::String { width: 8 }).unwrap();
    let Column::Text(t) = d.column(VarId(0)) else { panic!() };
    assert_eq!(t, &["1.50", "", "1234567.89"]);
    assert_eq!(d.var(VarId(0)).var_type, VarType::String { width: 10 });
    d.set_type(VarId(0), VarType::Numeric).unwrap();
    let v = d.numeric(VarId(0));
    assert_eq!(v[0], 1.5);
    assert!(v[1].is_nan());
    assert_eq!(v[2], 1234567.89);

    let mut d = Dataset::new();
    d.add_variable(Variable::string("t", 8), Column::Text(vec![" 4 ".into(), "abc".into(), "-0".into(), "".into()]))
        .unwrap();
    d.set_type(VarId(0), VarType::Numeric).unwrap();
    let v = d.numeric(VarId(0));
    assert_eq!(v[0], 4.0);
    assert!(v[1].is_nan() && v[3].is_nan());
    assert!(v[2] == 0.0 && v[2].is_sign_positive());
    assert!(d.set_type(VarId(3), VarType::Numeric).is_err());
}

/// String -> Numeric reports how many non-blank cells could not be parsed (became system-missing).
#[test]
fn set_type_reports_cells_lost() {
    let mut d = Dataset::new();
    d.add_variable(Variable::string("t", 8), Column::Text(vec!["1".into(), "abc".into(), "".into(), "78,5".into()]))
        .unwrap();
    assert_eq!(d.set_type(VarId(0), VarType::Numeric).unwrap(), 2);
    assert_eq!(d.set_type(VarId(0), VarType::Numeric).unwrap(), 0); // same type: no-op
}

/// The requested string width is honoured (never narrower than the longest formatted value).
#[test]
fn set_type_honours_requested_string_width() {
    let mut d = Dataset::new();
    d.add_variable(Variable::numeric("n"), Column::Numeric(vec![1.0, 12345.678])).unwrap();
    assert_eq!(d.set_type(VarId(0), VarType::String { width: 20 }).unwrap(), 0);
    assert_eq!(d.var(VarId(0)).var_type, VarType::String { width: 20 });
    let mut d = Dataset::new();
    d.add_variable(Variable::numeric("n"), Column::Numeric(vec![12345.678])).unwrap();
    d.set_type(VarId(0), VarType::String { width: 3 }).unwrap();
    assert_eq!(d.var(VarId(0)).var_type, VarType::String { width: 8 }); // "12345.68" is 8 bytes
}

/// `VarType::String { width }` is the storage width; changing it truncates longer values (on a
/// character boundary) and reports how many were cut. `Variable::width` is the display width.
#[test]
fn set_string_width_truncates_and_counts() {
    let mut d = Dataset::new();
    d.add_variable(Variable::string("s", 10), Column::Text(vec!["abcdef".into(), "ab".into(), "ééé".into()])).unwrap();
    assert_eq!(d.set_string_width(VarId(0), 4).unwrap(), 2);
    assert_eq!(d.var(VarId(0)).var_type, VarType::String { width: 4 });
    assert_eq!(d.var(VarId(0)).width, 10, "display width is unchanged");
    let Column::Text(t) = d.column(VarId(0)) else { panic!() };
    assert_eq!(t, &["abcd", "ab", "éé"]);
    assert_eq!(d.set_string_width(VarId(0), 30).unwrap(), 0);
    assert_eq!(d.var(VarId(0)).var_type, VarType::String { width: 30 });
    d.add_variable(Variable::numeric("n"), Column::Numeric(vec![1.0; 3])).unwrap();
    assert!(d.set_string_width(VarId(1), 4).is_err());
    assert!(d.set_string_width(VarId(0), 0).is_err());
    assert!(d.set_string_width(VarId(0), 32768).is_err());
    assert!(d.set_string_width(VarId(9), 4).is_err());
}

/// String -> String through set_type changes the storage width like set_string_width.
#[test]
fn set_type_string_to_string_changes_width() {
    let mut d = Dataset::new();
    d.add_variable(Variable::string("s", 6), Column::Text(vec!["abcdef".into()])).unwrap();
    assert_eq!(d.set_type(VarId(0), VarType::String { width: 3 }).unwrap(), 1);
    assert_eq!(d.var(VarId(0)).var_type, VarType::String { width: 3 });
}

/// Dataset-level metadata kept from .sav files: weight variable, file label, documents.
#[test]
fn weight_follows_rename_and_is_cleared_by_delete_and_type_change() {
    let mut d = ds();
    assert_eq!(d.weight(), None);
    assert!(d.set_weight(Some(VarId(1))).is_err(), "string weight");
    d.set_weight(Some(VarId(0))).unwrap();
    assert_eq!(d.weight(), Some(VarId(0)));
    d.insert_variable(0, Variable::numeric("z"), Column::Numeric(vec![0.0; 3])).unwrap();
    assert_eq!(d.weight(), Some(VarId(1)));
    d.rename_variable(VarId(1), "bobot").unwrap();
    assert_eq!(d.weight(), Some(VarId(1)));
    d.set_type(VarId(1), VarType::String { width: 8 }).unwrap();
    assert_eq!(d.weight(), None);
    d.set_type(VarId(1), VarType::Numeric).unwrap();
    assert_eq!(d.weight(), None, "type change cleared it");
    d.set_weight(Some(VarId(1))).unwrap();
    d.delete_variable(VarId(1)).unwrap();
    assert_eq!(d.weight(), None);
    d.add_variable(Variable::numeric("bobot"), Column::Numeric(vec![1.0; 3])).unwrap();
    assert_eq!(d.weight(), None, "a new variable with the old name is not the weight");
    assert!(d.set_weight(Some(VarId(99))).is_err());

    d.set_file_label(Some("Survei".into()));
    assert_eq!(d.file_label(), Some("Survei"));
    d.set_documents(vec!["baris 1".into()]);
    assert_eq!(d.documents(), ["baris 1"]);
}

/// Variable fields that only exist so .sav metadata survives a round trip.
#[test]
fn variable_has_sav_metadata_fields() {
    let mut v = Variable::string("s", 8);
    v.string_value_labels.insert("L".into(), "Laki-laki".into());
    v.string_missing.push("X".into());
    assert_eq!(v.format_type, None);
    let mut n = Variable::numeric("tgl");
    n.format_type = Some(20);
    assert!(n.has_date_format());
    assert!(!Variable::numeric("x").has_date_format());
}

#[test]
fn bulk_cases() {
    let mut d = ds();
    d.insert_cases(1, 2);
    assert_eq!(d.n_cases(), 5);
    assert_eq!(d.numeric(VarId(0))[0], 1.0);
    assert!(d.numeric(VarId(0))[1].is_nan() && d.numeric(VarId(0))[2].is_nan());
    let Column::Text(t) = d.column(VarId(1)) else { panic!() };
    assert_eq!(t, &["x", "", "", "y", "z"]);
    d.insert_cases(99, 1); // clamped to the end
    assert_eq!(d.n_cases(), 6);
    d.delete_cases(1..3).unwrap();
    d.delete_cases(3..4).unwrap();
    assert_eq!(d.numeric(VarId(0)), &[1.0, 2.0, 3.0]);
    assert!(d.delete_cases(2..4).is_err());
    assert!(d.delete_cases(2..2).is_err());
    assert_eq!(d.n_cases(), 3);
}
