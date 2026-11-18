use dataset::*;
use transform::*;

const NAN: f64 = f64::NAN;

fn ds_with(vals: Vec<f64>, missing: MissingSpec) -> Dataset {
    let mut ds = Dataset::new();
    let mut v = Variable::numeric("X");
    v.missing = missing;
    ds.add_variable(v, Column::Numeric(vals)).unwrap();
    ds
}

fn spec(out: &str, rules: Vec<(OldValue, NewValue)>) -> RecodeSpec {
    RecodeSpec { pairs: vec![(VarId(0), out.into(), None)], rules }
}

fn run(ds: &mut Dataset, s: &RecodeSpec) -> Vec<f64> {
    let ids = recode(ds, s).unwrap();
    ds.numeric(ids[0]).to_vec()
}

fn same(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x.is_nan() && y.is_nan()) || x == y)
}

#[test]
fn first_matching_rule_wins_on_overlap() {
    let mut ds = ds_with(vec![1.0, 4.0, 8.0, 11.0], MissingSpec::None);
    let s = spec(
        "Y",
        vec![
            (OldValue::Range(1.0, 5.0), NewValue::Value(1.0)),
            (OldValue::Range(3.0, 10.0), NewValue::Value(2.0)),
        ],
    );
    let out = run(&mut ds, &s);
    assert!(same(&out, &[1.0, 1.0, 2.0, NAN]));
    assert_eq!(ds.var(ds.find("Y").unwrap()).decimals, 0);
    assert_eq!(ds.var(ds.find("Y").unwrap()).width, 8);
}

#[test]
fn lowest_highest_else_copy() {
    let mut ds = ds_with(vec![1.0, 2.0, 5.0, 8.0, 9.0], MissingSpec::None);
    let s = spec(
        "Y",
        vec![
            (OldValue::Lowest(2.0), NewValue::Value(1.0)),
            (OldValue::Highest(8.0), NewValue::Value(3.0)),
            (OldValue::Else, NewValue::Copy),
        ],
    );
    assert!(same(&run(&mut ds, &s), &[1.0, 1.0, 5.0, 3.0, 3.0]));
}

#[test]
fn sysmis_and_missing() {
    let mut ds = ds_with(vec![1.0, NAN, 99.0], MissingSpec::Discrete(vec![99.0]));
    let s = spec(
        "Y",
        vec![
            (OldValue::SysMis, NewValue::Value(99.0)),
            (OldValue::Missing, NewValue::Value(9.0)),
            (OldValue::Else, NewValue::Copy),
        ],
    );
    assert!(same(&run(&mut ds, &s), &[1.0, 99.0, 9.0]));
}

#[test]
fn explicit_value_matches_user_missing_and_range_too() {
    let mut ds = ds_with(vec![99.0, 98.0], MissingSpec::Discrete(vec![99.0, 98.0]));
    let s = spec("Y", vec![(OldValue::Value(99.0), NewValue::Value(1.0)), (OldValue::Range(90.0, 98.5), NewValue::Value(2.0))]);
    assert!(same(&run(&mut ds, &s), &[1.0, 2.0]));
}

#[test]
fn no_match_is_nan_and_decimals_two_for_fractions() {
    let mut ds = ds_with(vec![1.0, 2.0], MissingSpec::None);
    let s = spec("Y", vec![(OldValue::Value(1.0), NewValue::Value(0.5))]);
    assert!(same(&run(&mut ds, &s), &[0.5, NAN]));
    assert_eq!(ds.var(ds.find("Y").unwrap()).decimals, 2);
}

#[test]
fn sysmis_new_value_and_negative_zero() {
    let mut ds = ds_with(vec![1.0, 2.0], MissingSpec::None);
    let s = spec("Y", vec![(OldValue::Value(1.0), NewValue::SysMis), (OldValue::Else, NewValue::Value(-0.0))]);
    let out = run(&mut ds, &s);
    assert!(out[0].is_nan());
    assert!(out[1] == 0.0 && out[1].is_sign_positive());
}

#[test]
fn copied_user_missing_has_no_missing_spec_and_label_set() {
    let mut ds = ds_with(vec![99.0], MissingSpec::Discrete(vec![99.0]));
    let mut s = spec("Y", vec![(OldValue::Else, NewValue::Copy)]);
    s.pairs[0].2 = Some("lbl".into());
    assert!(same(&run(&mut ds, &s), &[99.0]));
    let v = ds.var(ds.find("Y").unwrap());
    assert_eq!(v.missing, MissingSpec::None);
    assert_eq!(v.label.as_deref(), Some("lbl"));
}

#[test]
fn errors_leave_dataset_unchanged() {
    let mut ds = ds_with(vec![1.0], MissingSpec::None);
    ds.add_variable(Variable::string("S", 4), Column::Text(vec!["a".into()])).unwrap();
    let r = vec![(OldValue::Else, NewValue::Copy)];
    // existing name (case-insensitive)
    assert!(recode(&mut ds, &spec("x", r.clone())).is_err());
    // invalid name
    assert!(recode(&mut ds, &spec("1bad", r.clone())).is_err());
    // no rules
    assert!(recode(&mut ds, &spec("Y", vec![])).is_err());
    // string input
    let s = RecodeSpec { pairs: vec![(VarId(1), "Y".into(), None)], rules: r.clone() };
    assert!(recode(&mut ds, &s).is_err());
    // stale id
    let s = RecodeSpec { pairs: vec![(VarId(9), "Y".into(), None)], rules: r.clone() };
    assert!(recode(&mut ds, &s).is_err());
    // duplicate outputs; second pair invalid input must not leave first created
    let s = RecodeSpec { pairs: vec![(VarId(0), "Y".into(), None), (VarId(0), "y".into(), None)], rules: r.clone() };
    assert!(recode(&mut ds, &s).is_err());
    assert_eq!(ds.n_vars(), 2);
}

#[test]
fn multiple_pairs() {
    let mut ds = ds_with(vec![1.0, 2.0], MissingSpec::None);
    let s = RecodeSpec {
        pairs: vec![(VarId(0), "A".into(), None), (VarId(0), "B".into(), None)],
        rules: vec![(OldValue::Value(1.0), NewValue::Value(5.0)), (OldValue::Else, NewValue::Copy)],
    };
    let ids = recode(&mut ds, &s).unwrap();
    assert_eq!(ids.len(), 2);
    assert!(same(ds.numeric(ids[1]), &[5.0, 2.0]));
}

fn var_names(ds: &Dataset) -> Vec<String> {
    (0..ds.n_vars()).map(|i| ds.var(VarId(i)).name.clone()).collect()
}

/// Output names that differ only in non-ASCII case are duplicates (as `Dataset::find` sees
/// them): the recode fails and the dataset is unchanged.
#[test]
fn non_ascii_case_duplicate_outputs_leave_dataset_unchanged() {
    let mut ds = ds_with(vec![1.0, 2.0], MissingSpec::None);
    let s = RecodeSpec {
        pairs: vec![(VarId(0), "Äx".into(), None), (VarId(0), "äx".into(), None)],
        rules: vec![(OldValue::Else, NewValue::Copy)],
    };
    assert!(matches!(recode(&mut ds, &s), Err(TransformError::NameTaken(_))));
    assert_eq!(ds.n_vars(), 1);
    assert_eq!(var_names(&ds), ["X"]);
}

#[test]
fn nan_bounds_and_reversed_ranges_are_rejected() {
    let bad = [
        OldValue::Value(NAN),
        OldValue::Range(NAN, 1.0),
        OldValue::Range(1.0, NAN),
        OldValue::Lowest(NAN),
        OldValue::Highest(NAN),
        OldValue::Range(5.0, 1.0),
    ];
    for old in bad {
        let mut ds = ds_with(vec![1.0, 2.0], MissingSpec::None);
        let r = recode(&mut ds, &spec("Y", vec![(old, NewValue::Value(1.0))]));
        assert!(matches!(r, Err(TransformError::InvalidArguments(_))), "{old:?}: {r:?}");
        assert_eq!(var_names(&ds), ["X"], "{old:?}");
    }
    // lo == hi is a valid one-value range.
    let mut ds = ds_with(vec![1.0, 2.0], MissingSpec::None);
    assert!(recode(&mut ds, &spec("Y", vec![(OldValue::Range(1.0, 1.0), NewValue::Value(9.0))])).is_ok());
}
