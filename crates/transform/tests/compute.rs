use dataset::*;
use transform::*;

const NAN: f64 = f64::NAN;

fn ds_with(vars: &[(&str, Vec<f64>)]) -> Dataset {
    let mut ds = Dataset::new();
    for (name, vals) in vars {
        ds.add_variable(Variable::numeric(name), Column::Numeric(vals.clone())).unwrap();
    }
    ds
}

fn spec(target: &str, expression: &str) -> ComputeSpec {
    ComputeSpec { target: target.into(), expression: expression.into(), condition: None, label: None }
}

fn col(ds: &Dataset, name: &str) -> Vec<f64> {
    ds.numeric(ds.find(name).unwrap()).to_vec()
}

/// Same values, NaN == NaN, -0.0 distinct from 0.0 (to check normalisation).
fn same(got: &[f64], want: &[f64]) {
    assert_eq!(got.len(), want.len(), "got {got:?}, want {want:?}");
    for (g, w) in got.iter().zip(want) {
        let ok = (g.is_nan() && w.is_nan()) || (g == w && g.is_sign_negative() == w.is_sign_negative());
        assert!(ok, "got {got:?}, want {want:?}");
    }
}

/// Evaluate a constant expression on a one-case dataset.
fn eval1(src: &str) -> f64 {
    let mut ds = ds_with(&[("dummy", vec![1.0])]);
    compute(&mut ds, &spec("r", src)).unwrap_or_else(|e| panic!("{src}: {e}"));
    col(&ds, "r")[0]
}

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-12, "{a} vs {b}");
}

// ---------- brief: Step 1 ----------

#[test]
fn precedence_power_binds_tighter_than_unary_minus() {
    assert_eq!(eval1("2+3*4**2"), 50.0);
    assert_eq!(eval1("-2**2"), -4.0);
}

#[test]
fn abs_of_residual_keeps_missing_rows() {
    let mut ds = ds_with(&[("RES_1", vec![-1.5, NAN, 2.0, -0.0])]);
    let rep = compute(&mut ds, &spec("ABS_RES", "ABS(RES_1)")).unwrap();
    assert!(rep.created);
    assert_eq!(rep.n_sysmis, 1);
    same(&col(&ds, "ABS_RES"), &[1.5, NAN, 2.0, 0.0]);
    let v = ds.var(rep.target);
    assert_eq!((v.name.as_str(), v.width, v.decimals), ("ABS_RES", 8, 2));
    assert_eq!(v.var_type, VarType::Numeric);
}

#[test]
fn arithmetic_with_missing_is_sysmis() {
    let mut ds = ds_with(&[("x", vec![1.0, 2.0, 3.0]), ("y", vec![10.0, NAN, 99.0])]);
    let y = ds.find("y").unwrap();
    ds.var_mut(y).missing = MissingSpec::Discrete(vec![99.0]);
    compute(&mut ds, &spec("z", "x + y")).unwrap();
    same(&col(&ds, "z"), &[11.0, NAN, NAN]);
}

#[test]
fn mean_uses_valid_values_only() {
    let mut ds = ds_with(&[("x", vec![4.0, NAN]), ("y", vec![NAN, NAN])]);
    compute(&mut ds, &spec("m", "MEAN(x,y)")).unwrap();
    same(&col(&ds, "m"), &[4.0, NAN]);
}

#[test]
fn division_by_zero_and_ln_zero_are_sysmis() {
    let mut ds = ds_with(&[("y", vec![5.0, 0.0])]);
    compute(&mut ds, &spec("d", "y/0")).unwrap();
    compute(&mut ds, &spec("l", "LN(y)")).unwrap();
    same(&col(&ds, "d"), &[NAN, NAN]);
    same(&col(&ds, "l"), &[5f64.ln(), NAN]);
    assert!(eval1("LN(0)").is_nan());
}

#[test]
fn three_valued_and() {
    let mut ds = ds_with(&[("x", vec![2.0, 5.0, 5.0]), ("y", vec![NAN, NAN, 1.0])]);
    compute(&mut ds, &spec("c", "x > 3 AND y")).unwrap();
    same(&col(&ds, "c"), &[0.0, NAN, 1.0]);
}

#[test]
fn condition_fills_only_matching_rows() {
    let mut ds = ds_with(&[("grp", vec![1.0, 2.0, 1.0, NAN]), ("x", vec![10.0, 20.0, 30.0, 40.0])]);
    let s = ComputeSpec { condition: Some("grp = 1".into()), ..spec("z", "x * 2") };
    let rep = compute(&mut ds, &s).unwrap();
    same(&col(&ds, "z"), &[20.0, NAN, 60.0, NAN]);
    assert_eq!(rep.n_sysmis, 2);
    // Existing variable: rows not matching keep their old values.
    let s = ComputeSpec { condition: Some("grp = 2".into()), ..spec("x", "0") };
    let rep = compute(&mut ds, &s).unwrap();
    assert!(!rep.created);
    same(&col(&ds, "x"), &[10.0, 0.0, 30.0, 40.0]);
}

#[test]
fn invalid_target_name_is_error() {
    let mut ds = ds_with(&[("x", vec![1.0])]);
    for bad in ["1abc", "", "a b", "x-y", "AND", "to"] {
        let e = compute(&mut ds, &spec(bad, "x")).unwrap_err();
        assert!(matches!(e, TransformError::InvalidTarget(_)), "{bad}: {e:?}");
    }
    assert_eq!(ds.n_vars(), 1);
}

#[test]
fn syntax_error_has_position() {
    let e = parse("ABS(x").unwrap_err();
    assert_eq!(e.pos, 5);
    assert_eq!(e.message, "Expected ')'");
    let e = parse("1 + * 2").unwrap_err();
    assert_eq!(e.pos, 4);
    let e = parse("FOO(1)").unwrap_err();
    assert_eq!((e.pos, e.message.as_str()), (0, "Unknown function FOO"));
    let mut ds = ds_with(&[("x", vec![1.0])]);
    match compute(&mut ds, &spec("z", "x +")).unwrap_err() {
        TransformError::Parse(p) => assert_eq!(p.pos, 3),
        e => panic!("{e:?}"),
    }
    assert_eq!(ds.n_vars(), 1, "no target created on error");
}

#[test]
fn lag_first_case_is_sysmis() {
    let mut ds = ds_with(&[("x", vec![1.0, 2.0, 3.0])]);
    compute(&mut ds, &spec("l", "LAG(x)")).unwrap();
    same(&col(&ds, "l"), &[NAN, 1.0, 2.0]);
}

// ---------- parsing details ----------

#[test]
fn parse_builds_expected_tree() {
    let e = parse("-a ** 2").unwrap();
    assert_eq!(
        e,
        Expr::Unary(
            Op::Neg,
            Box::new(Expr::Binary(Op::Pow, Box::new(Expr::Var("a".into())), Box::new(Expr::Num(2.0))))
        )
    );
    assert_eq!(parse("mean(a, 'x')").unwrap(), Expr::Call("MEAN".into(), vec![Expr::Var("a".into()), Expr::Str("x".into())]));
}

#[test]
fn power_is_left_associative_and_accepts_negative_exponent() {
    assert_eq!(eval1("2**3**2"), 64.0);
    assert_eq!(eval1("2**-1"), 0.5);
    assert_eq!(eval1("(-2)**2"), 4.0);
    assert!(eval1("(-8)**(1/3)").is_nan());
    assert!(eval1("0**-1").is_nan());
}

#[test]
fn numbers_keywords_and_operators() {
    assert_eq!(eval1(".5 + 1e3 + 2.5E-1 + 1."), 1001.75);
    assert_eq!(eval1("7 - 2 - 1"), 4.0);
    assert_eq!(eval1("8 / 4 / 2"), 1.0);
    assert_eq!(eval1("1 + 2 * 3 > 6 and not 0"), 1.0);
    assert_eq!(eval1("1 eq 1 & 2 GE 3 | 3 lt 4"), 1.0);
    assert_eq!(eval1("~(1 = 1)"), 0.0);
    assert_eq!(eval1("1 ~= 2"), 1.0);
    assert_eq!(eval1("1 <> 1"), 0.0);
    assert_eq!(eval1("2 <= 2 AND 2 >= 3"), 0.0);
    assert_eq!(eval1("1 NE 2 AND 1 LE 2 AND 3 GT 2"), 1.0);
    assert_eq!(eval1("NOT 1 = 2"), 1.0, "NOT binds looser than comparison");
    assert_eq!(eval1("1 OR 0 AND 0"), 1.0, "AND binds tighter than OR");
    assert_eq!(eval1("-3 + +2"), -1.0);
    assert_eq!(eval1("abs(-3) + Sqrt(16)"), 7.0);
}

#[test]
fn variables_are_case_insensitive() {
    let mut ds = ds_with(&[("Score", vec![2.0])]);
    compute(&mut ds, &spec("SCORE2", "score * SCORE")).unwrap();
    assert_eq!(col(&ds, "score2"), vec![4.0]);
}

#[test]
fn error_messages() {
    let mut ds = ds_with(&[("x", vec![1.0])]);
    let e = compute(&mut ds, &spec("z", "x + bar")).unwrap_err();
    assert!(matches!(e, TransformError::UnknownVariable(ref n) if n == "bar"), "{e:?}");
    assert_eq!(e.to_string(), "Unknown variable bar");
    assert!(compute(&mut ds, &spec("z", "ABS(1, 2)")).unwrap_err().to_string().contains("ABS"));
    assert!(parse("SQRT()").is_err());
    assert!(parse("MEAN()").is_err());
    assert!(parse("1 2").is_err());
    assert!(parse("").is_err());
    assert!(parse("'abc").is_err());
    assert!(parse("x TO y").is_err());
    assert!(parse("1e999").is_err());
    assert!(parse("ABS.2(x)").is_err());
    assert!(parse("MEAN.0(x)").is_err());
}

// ---------- functions ----------

#[test]
fn scalar_functions() {
    assert_eq!(eval1("RND(2.5)"), 3.0);
    assert_eq!(eval1("RND(-2.5)"), -3.0);
    assert_eq!(eval1("RND(2.4999999999999996)"), 3.0, "SPSS fuzz");
    assert_eq!(eval1("RND(17, 5)"), 15.0);
    assert_eq!(eval1("RND(2.71828, 0.01)"), 2.72);
    assert!(eval1("RND(3, 0)").is_nan());
    same(&[eval1("RND(-0.4)")], &[0.0]);
    assert_eq!(eval1("TRUNC(2.7)"), 2.0);
    assert_eq!(eval1("TRUNC(-2.7)"), -2.0);
    same(&[eval1("TRUNC(-0.7)")], &[0.0]);
    assert_eq!(eval1("MOD(-7, 3)"), -1.0);
    assert_eq!(eval1("MOD(7, -3)"), 1.0);
    assert!(eval1("MOD(7, 0)").is_nan());
    close(eval1("LG10(1000)"), 3.0);
    close(eval1("EXP(1)"), std::f64::consts::E);
    assert!(eval1("EXP(1000)").is_nan());
    assert!(eval1("SQRT(-1)").is_nan());
    assert!(eval1("LG10(-1)").is_nan());
    assert!(eval1("1e300 * 1e300").is_nan());
    same(&[eval1("0 * -1")], &[0.0]);
}

#[test]
fn statistical_functions() {
    let mut ds = ds_with(&[("a", vec![1.0, 1.0, NAN]), ("b", vec![2.0, NAN, NAN]), ("c", vec![6.0, NAN, NAN])]);
    for (name, src) in [
        ("sum", "SUM(a,b,c)"),
        ("mean", "MEAN(a,b,c)"),
        ("mean2", "MEAN.2(a,b,c)"),
        ("sd", "SD(a,b,c)"),
        ("var", "VARIANCE(a,b,c)"),
        ("mn", "MIN(a,b,c)"),
        ("mx", "MAX(a,b,c)"),
        ("nv", "NVALID(a,b,c)"),
        ("rng", "MEAN(a TO c)"),
        ("mixed", "SUM(a TO b, 10)"),
    ] {
        compute(&mut ds, &spec(name, src)).unwrap_or_else(|e| panic!("{src}: {e}"));
    }
    same(&col(&ds, "sum"), &[9.0, 1.0, NAN]);
    same(&col(&ds, "mean"), &[3.0, 1.0, NAN]);
    same(&col(&ds, "mean2"), &[3.0, NAN, NAN]);
    let sd = col(&ds, "sd");
    close(sd[0], 7f64.sqrt());
    assert!(sd[1].is_nan() && sd[2].is_nan(), "SD needs two valid values");
    close(col(&ds, "var")[0], 7.0);
    same(&col(&ds, "mn"), &[1.0, 1.0, NAN]);
    same(&col(&ds, "mx"), &[6.0, 1.0, NAN]);
    same(&col(&ds, "nv"), &[3.0, 1.0, 0.0]);
    same(&col(&ds, "rng"), &[3.0, 1.0, NAN]);
    same(&col(&ds, "mixed"), &[13.0, 11.0, 10.0]);

    // TO range in reverse dataset order, or over a string variable, is an error.
    assert!(compute(&mut ds, &spec("z", "MEAN(c TO a)")).is_err());
    // .n larger than the number of arguments.
    assert!(compute(&mut ds, &spec("z", "MEAN.4(a,b,c)")).is_err());
}

#[test]
fn missing_and_sysmis() {
    let mut ds = ds_with(&[("x", vec![1.0, NAN, 99.0])]);
    let x = ds.find("x").unwrap();
    ds.var_mut(x).missing = MissingSpec::Discrete(vec![99.0]);
    compute(&mut ds, &spec("m", "MISSING(x)")).unwrap();
    compute(&mut ds, &spec("s", "SYSMIS(x)")).unwrap();
    compute(&mut ds, &spec("e", "SYSMIS(x + 1)")).unwrap();
    compute(&mut ds, &spec("o", "x > 0 OR 1")).unwrap();
    compute(&mut ds, &spec("o2", "x > 0 OR 0")).unwrap();
    compute(&mut ds, &spec("n", "NOT x")).unwrap();
    same(&col(&ds, "m"), &[0.0, 1.0, 1.0]);
    same(&col(&ds, "s"), &[0.0, 1.0, 0.0]);
    same(&col(&ds, "e"), &[0.0, 1.0, 1.0]);
    same(&col(&ds, "o"), &[1.0, 1.0, 1.0]);
    same(&col(&ds, "o2"), &[1.0, NAN, NAN]);
    same(&col(&ds, "n"), &[0.0, NAN, NAN]);
}

#[test]
fn lag_sees_values_computed_for_earlier_cases() {
    // SPSS evaluates case by case: a cumulative sum via LAG of the target itself.
    let mut ds = ds_with(&[("v", vec![1.0, 2.0, 3.0]), ("cum", vec![0.0, 0.0, 0.0])]);
    compute(&mut ds, &spec("cum", "SUM(LAG(cum), v)")).unwrap();
    same(&col(&ds, "cum"), &[1.0, 3.0, 6.0]);
}

// ---------- strings ----------

#[test]
fn string_conditions() {
    let mut ds = ds_with(&[("x", vec![1.0, 2.0, 3.0])]);
    ds.add_variable(Variable::string("gender", 4), Column::Text(vec!["L   ".into(), "P".into(), "L".into()])).unwrap();
    let s = ComputeSpec { condition: Some("gender = 'L'".into()), ..spec("z", "x") };
    compute(&mut ds, &s).unwrap();
    same(&col(&ds, "z"), &[1.0, NAN, 3.0]);
    compute(&mut ds, &spec("p", "gender <> \"L\" OR gender < 'M'")).unwrap();
    same(&col(&ds, "p"), &[1.0, 1.0, 1.0]);
    compute(&mut ds, &spec("q", "'it''s' = \"it's\"")).unwrap();
    same(&col(&ds, "q"), &[1.0, 1.0, 1.0]);
}

#[test]
fn string_type_errors() {
    let mut ds = ds_with(&[("x", vec![1.0])]);
    ds.add_variable(Variable::string("gender", 1), Column::Text(vec!["L".into()])).unwrap();
    for src in ["gender + 1", "ABS(gender)", "gender = 1", "'a'", "gender", "MEAN(x TO gender)", "-gender", "gender AND 1"] {
        let e = compute(&mut ds, &spec("z", src)).unwrap_err();
        assert!(matches!(e, TransformError::Type(_)), "{src}: {e:?}");
    }
    let e = compute(&mut ds, &spec("z", "gender + 1")).unwrap_err();
    assert!(e.to_string().contains("gender"), "{e}");
    // String target is not supported.
    let e = compute(&mut ds, &spec("GENDER", "1")).unwrap_err();
    assert!(matches!(e, TransformError::InvalidTarget(_)), "{e:?}");
    assert_eq!(ds.n_vars(), 2);
}

// ---------- target handling ----------

#[test]
fn overwrite_keeps_metadata_and_label_applies() {
    let mut ds = ds_with(&[("x", vec![1.0, 2.0])]);
    let x = ds.find("x").unwrap();
    ds.var_mut(x).label = Some("Old".into());
    ds.var_mut(x).decimals = 0;
    let rep = compute(&mut ds, &spec("X", "x * 10")).unwrap();
    assert_eq!((rep.target, rep.created, rep.n_sysmis), (x, false, 0));
    assert_eq!(ds.var(x).label.as_deref(), Some("Old"));
    assert_eq!(ds.var(x).decimals, 0);
    assert_eq!(ds.var(x).name, "x");
    same(&col(&ds, "x"), &[10.0, 20.0]);

    let s = ComputeSpec { label: Some("Absolute residual".into()), ..spec("x", "x") };
    compute(&mut ds, &s).unwrap();
    assert_eq!(ds.var(x).label.as_deref(), Some("Absolute residual"));
    let s = ComputeSpec { label: Some("New".into()), ..spec("y", "x") };
    let rep = compute(&mut ds, &s).unwrap();
    assert_eq!(ds.var(rep.target).label.as_deref(), Some("New"));
}

#[test]
fn condition_errors() {
    let mut ds = ds_with(&[("x", vec![1.0])]);
    let s = ComputeSpec { condition: Some("x >".into()), ..spec("z", "1") };
    assert!(matches!(compute(&mut ds, &s).unwrap_err(), TransformError::ConditionParse(_)));
    let s = ComputeSpec { condition: Some("nope = 1".into()), ..spec("z", "1") };
    assert!(matches!(compute(&mut ds, &s).unwrap_err(), TransformError::UnknownVariable(_)));
    assert_eq!(ds.n_vars(), 1);
}

#[test]
fn empty_dataset() {
    let mut ds = Dataset::new();
    let rep = compute(&mut ds, &spec("z", "1 + 1")).unwrap();
    assert_eq!((rep.created, rep.n_sysmis, ds.n_cases()), (true, 0, 0));
}

// ---------- robustness ----------

#[test]
fn deep_nesting_is_an_error_not_a_crash() {
    let deep = format!("{}1{}", "(".repeat(100_000), ")".repeat(100_000));
    assert!(parse(&deep).is_err());
    assert!(parse(&"-".repeat(100_000)).is_err());
    assert!(parse(&format!("{}1", "NOT ".repeat(100_000))).is_err());
    assert!(parse(&format!("{}1", "ABS(".repeat(100_000))).is_err());
    let chain = vec!["1"; 100_000].join("+");
    assert!(parse(&chain).is_err());
    let pow = vec!["1"; 100_000].join("**");
    assert!(parse(&pow).is_err());
    // Nesting just inside the limits works, even on a 1 MB stack (the Windows main thread).
    let on_small_stack = |f: fn()| std::thread::Builder::new().stack_size(1 << 20).spawn(f).unwrap().join().unwrap();
    on_small_stack(|| {
        let parens = format!("{}1{}", "(".repeat(200), ")".repeat(200));
        assert_eq!(eval1(&parens), 1.0);
        let sum = vec!["1"; 500].join("+");
        assert_eq!(eval1(&sum), 500.0);
        let neg = format!("{}1", "-".repeat(200));
        assert_eq!(eval1(&neg), 1.0);
        let mixed = format!("{}{}{}", "ABS(".repeat(199), vec!["1"; 300].join("+"), ")".repeat(199));
        assert_eq!(eval1(&mixed), 300.0);
    });
    assert!(parse(&format!("{}1{}", "(".repeat(201), ")".repeat(201))).is_err());
    assert!(parse(&vec!["1"; 502].join("+")).is_err());
}

#[test]
fn garbage_input_never_panics() {
    let pieces = [
        "(", ")", "+", "-", "*", "/", "**", "=", "<>", "~=", "~", "<", ">=", "&", "|", ",", "'", "\"", ".", "1", ".5",
        "1e", "1e3", "x", "y", "s", "AND", "or", "NOT", "TO", "MEAN", "mean.2", "ABS", "LAG", "RND", "MISSING", "é", "ß",
        " ", "\t", "\n", "#", "@", "$", "_", "!", "%", "\u{0}", "日本", "999999999999999999999",
    ];
    let mut seed: u64 = 0x2545_F491_4F6C_DD1D;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    let mut ds = ds_with(&[("x", vec![1.0, NAN, -3.0]), ("y", vec![0.0, 2.0, NAN])]);
    ds.add_variable(Variable::string("s", 2), Column::Text(vec!["a".into(), "".into(), "é".into()])).unwrap();
    for _ in 0..20_000 {
        let len = (next() % 12) as usize;
        let src: String = (0..len).map(|_| pieces[(next() % pieces.len() as u64) as usize]).collect();
        let _ = parse(&src);
        let s = ComputeSpec { condition: Some(src.clone()), ..spec("g", &src) };
        let _ = compute(&mut ds, &s);
        if let Err(e) = parse(&src) {
            assert!(e.pos <= src.len() && src.is_char_boundary(e.pos), "{src:?}: {e:?}");
            let _ = e.to_string();
        }
    }
    // Every byte sequence that is valid UTF-8 with odd characters.
    for src in ["\u{FEFF}x", "x\u{200B}", "ｘ", "1\u{2212}2", "'\u{1F600}'", "MEAN(x TO", "MEAN(x TO 1)", "LAG(1)"] {
        let _ = compute(&mut ds, &spec("g", src));
    }
}
