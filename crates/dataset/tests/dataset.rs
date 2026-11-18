use dataset::*;
use std::path::Path;

fn edge() -> Dataset {
    load_csv(Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/data/edge_cases.csv"))).unwrap()
}

#[test]
fn validate_names() {
    assert!(validate_name("x1").is_ok());
    assert!(validate_name("1x").is_err());
    assert!(validate_name("x y").is_err());
    assert!(validate_name(&"a".repeat(65)).is_err());
}

#[test]
fn duplicate_name_case_insensitive() {
    let mut ds = Dataset::new();
    ds.add_variable(Variable::numeric("x1"), Column::Numeric(vec![1.0])).unwrap();
    let r = ds.add_variable(Variable::numeric("X1"), Column::Numeric(vec![2.0]));
    assert!(matches!(r, Err(DatasetError::DuplicateName(_))));
}

#[test]
fn listwise_with_user_missing() {
    let mut ds = edge();
    assert_eq!(ds.n_cases(), 12);
    let id = ds.find("WITHMISS").unwrap();
    ds.var_mut(id).missing = MissingSpec::Discrete(vec![99.0]);
    let rows = ds.listwise_rows(&[id]);
    assert_eq!(rows.len(), 9);
    for r in [2, 4, 7] {
        assert!(!rows.contains(&r));
    }
    assert!(ds.is_missing(id, 4));
}

#[test]
fn csv_defaults() {
    let ds = edge();
    let y = ds.var(ds.find("y").unwrap());
    assert_eq!(y.decimals, 1);
    assert_eq!(y.width, 8);
    assert_eq!(y.measure, Measure::Scale);
}

#[test]
fn unique_name_next() {
    let mut ds = Dataset::new();
    ds.add_variable(Variable::numeric("RES_1"), Column::Numeric(vec![])).unwrap();
    assert_eq!(ds.unique_name("RES_"), "RES_2");
}

#[test]
fn range_missing() {
    let mut ds = Dataset::new();
    let mut v = Variable::numeric("a");
    v.missing = MissingSpec::Range { lo: 90.0, hi: 98.0, discrete: Some(-1.0) };
    let id = ds.add_variable(v, Column::Numeric(vec![1.0, 95.0, -1.0, 99.0])).unwrap();
    let m: Vec<bool> = (0..4).map(|i| ds.is_missing(id, i)).collect();
    assert_eq!(m, [false, true, true, false]);
}
