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

#[test]
fn ordered_value_negative_zero_is_zero() {
    assert_eq!(OrderedValue(-0.0), OrderedValue(0.0));
    assert_eq!(OrderedValue(-0.0).cmp(&OrderedValue(0.0)), std::cmp::Ordering::Equal);
    assert!(OrderedValue::new(-0.0).0.is_sign_positive());
    let mut m = std::collections::BTreeMap::new();
    m.insert(OrderedValue(0.0), "Nol");
    assert_eq!(m.get(&OrderedValue(-0.0)), Some(&"Nol"));
    assert!(OrderedValue(-1.0) < OrderedValue(-0.0));
}

/// SPSS limits names to 64 bytes (UTF-8), not 64 characters; reserved words are not names.
#[test]
fn name_length_is_counted_in_bytes_and_reserved_words_are_rejected() {
    assert!(validate_name(&"a".repeat(64)).is_ok());
    assert!(validate_name(&"é".repeat(32)).is_ok()); // 64 bytes
    assert!(validate_name(&"é".repeat(33)).is_err()); // 33 chars, 66 bytes
    for w in ["ALL", "and", "By", "EQ", "ge", "GT", "le", "LT", "ne", "NOT", "or", "TO", "with"] {
        assert!(validate_name(w).is_err(), "{w}");
    }
    assert!(validate_name("andy").is_ok());
    assert!(is_reserved_word("with") && !is_reserved_word("withx"));
}
