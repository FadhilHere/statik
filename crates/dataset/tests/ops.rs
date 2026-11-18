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
