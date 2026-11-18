//! Binding (name resolution + type checking) and per-case evaluation.

use crate::compute::TransformError;
use crate::functions::{self, Func, finite};
use crate::parser::{Expr, Op};
use dataset::{Column, Dataset, VarId, VarType};

/// String operand: literal or string variable.
#[derive(Debug)]
pub(crate) enum StrNode {
    Lit(String),
    Var(VarId),
}

/// A type-checked expression with variables resolved to ids. Always numeric-valued.
#[derive(Debug)]
pub(crate) enum Node {
    Num(f64),
    /// Numeric variable; missing (sysmis or user-missing) reads as NaN.
    Var(VarId),
    Neg(Box<Node>),
    Arith(Op, Box<Node>, Box<Node>),
    Cmp(Op, Box<Node>, Box<Node>),
    StrCmp(Op, StrNode, StrNode),
    And(Box<Node>, Box<Node>),
    Or(Box<Node>, Box<Node>),
    Not(Box<Node>),
    Scalar(Func, Vec<Node>),
    Stat { func: Func, min_valid: usize, args: Vec<Node> },
    Lag(VarId),
    MissingVar(VarId),
    MissingExpr(Box<Node>),
    SysmisVar(VarId),
    SysmisExpr(Box<Node>),
}

fn lookup(ds: &Dataset, name: &str) -> Result<VarId, TransformError> {
    ds.find(name).ok_or_else(|| TransformError::UnknownVariable(name.to_string()))
}

fn is_string(ds: &Dataset, id: VarId) -> bool {
    matches!(ds.var(id).var_type, VarType::String { .. })
}

fn is_comparison(op: Op) -> bool {
    matches!(op, Op::Eq | Op::Ne | Op::Lt | Op::Gt | Op::Le | Op::Ge)
}

/// Short description of an operand for type-error messages.
fn describe(e: &Expr) -> String {
    match e {
        Expr::Str(s) => format!("string '{s}'"),
        Expr::Var(n) => format!("string variable {n}"),
        _ => "expression".into(),
    }
}

/// `Some` if `e` is a string-valued operand (literal or string variable).
fn str_operand(ds: &Dataset, e: &Expr) -> Result<Option<StrNode>, TransformError> {
    Ok(match e {
        Expr::Str(s) => Some(StrNode::Lit(s.clone())),
        Expr::Var(n) => {
            let id = lookup(ds, n)?;
            is_string(ds, id).then_some(StrNode::Var(id))
        }
        _ => None,
    })
}

/// Bind a numeric-valued expression.
///
/// Recursive, so the frame is kept small: anything that formats messages or allocates
/// vectors lives in separate non-inlined helpers.
pub(crate) fn bind(ds: &Dataset, e: &Expr) -> Result<Node, TransformError> {
    Ok(match e {
        Expr::Num(v) => Node::Num(*v),
        Expr::Var(n) => bind_var(ds, n)?,
        Expr::Unary(Op::Neg, x) => Node::Neg(Box::new(bind(ds, x)?)),
        Expr::Unary(Op::Not, x) => Node::Not(Box::new(bind(ds, x)?)),
        Expr::Binary(op, l, r) if is_comparison(*op) => bind_cmp(ds, *op, l, r)?,
        Expr::Binary(op @ (Op::Add | Op::Sub | Op::Mul | Op::Div | Op::Pow | Op::And | Op::Or), l, r) => {
            let (l, r) = (Box::new(bind(ds, l)?), Box::new(bind(ds, r)?));
            match op {
                Op::And => Node::And(l, r),
                Op::Or => Node::Or(l, r),
                _ => Node::Arith(*op, l, r),
            }
        }
        Expr::Call(name, args) => bind_call(ds, name, args)?,
        _ => return Err(bad_expr(e)),
    })
}

#[cold]
#[inline(never)]
fn bad_expr(e: &Expr) -> TransformError {
    TransformError::Type(match e {
        Expr::Str(s) => format!("String '{s}' cannot be used where a number is expected"),
        Expr::Binary(Op::To, ..) => "TO is only allowed in arguments of statistical functions".into(),
        Expr::Unary(op, _) | Expr::Binary(op, ..) => format!("Invalid use of operator {op:?}"),
        _ => "Invalid expression".into(),
    })
}

#[inline(never)]
fn bind_var(ds: &Dataset, n: &str) -> Result<Node, TransformError> {
    let id = lookup(ds, n)?;
    if is_string(ds, id) {
        return Err(TransformError::Type(format!("String variable {n} cannot be used where a number is expected")));
    }
    Ok(Node::Var(id))
}

#[inline(never)]
fn bind_cmp(ds: &Dataset, op: Op, l: &Expr, r: &Expr) -> Result<Node, TransformError> {
    Ok(match (str_operand(ds, l)?, str_operand(ds, r)?) {
        (Some(a), Some(c)) => Node::StrCmp(op, a, c),
        (None, None) => Node::Cmp(op, Box::new(bind(ds, l)?), Box::new(bind(ds, r)?)),
        (Some(_), None) => {
            return Err(TransformError::Type(format!("Cannot compare {} with a number", describe(l))));
        }
        (None, Some(_)) => {
            return Err(TransformError::Type(format!("Cannot compare a number with {}", describe(r))));
        }
    })
}

#[inline(never)]
fn bind_call(ds: &Dataset, name: &str, args: &[Expr]) -> Result<Node, TransformError> {
    let fref = functions::resolve(name).map_err(TransformError::InvalidArguments)?;
    let func = fref.func;
    let (min, max) = func.arity();
    if args.len() < min || max.is_some_and(|m| args.len() > m) {
        return Err(TransformError::InvalidArguments(format!("Wrong number of arguments for {}", func.name())));
    }
    match func {
        Func::Lag | Func::Missing | Func::Sysmis => bind_var_function(ds, func, &args[0]),
        _ => {
            let mut nodes = Vec::with_capacity(args.len());
            for a in args {
                match a {
                    Expr::Binary(Op::To, first, last) => expand_range(ds, func, first, last, &mut nodes)?,
                    other => nodes.push(bind(ds, other)?),
                }
            }
            if !func.is_statistical() {
                return Ok(Node::Scalar(func, nodes));
            }
            if let Some(n) = fref.min_valid
                && n > nodes.len()
            {
                return Err(TransformError::InvalidArguments(format!(
                    "{}.{n} requires at least {n} arguments, got {}",
                    func.name(),
                    nodes.len()
                )));
            }
            let min_valid = fref.min_valid.unwrap_or(func.default_min_valid());
            Ok(Node::Stat { func, min_valid, args: nodes })
        }
    }
}

/// LAG / MISSING / SYSMIS, which look at a variable's raw value when given one.
#[inline(never)]
fn bind_var_function(ds: &Dataset, func: Func, arg: &Expr) -> Result<Node, TransformError> {
    let var = match arg {
        Expr::Var(n) => Some((n, lookup(ds, n)?)),
        _ => None,
    };
    Ok(match (func, var) {
        (Func::Lag, Some((n, id))) if is_string(ds, id) => {
            return Err(TransformError::Type(format!("LAG of string variable {n} is not supported")));
        }
        (Func::Lag, Some((_, id))) => Node::Lag(id),
        (Func::Lag, None) => return Err(TransformError::Type("LAG requires a variable name as its argument".into())),
        // String variables have no missing values.
        (Func::Missing, Some((_, id))) if is_string(ds, id) => Node::Num(0.0),
        (Func::Missing, Some((_, id))) => Node::MissingVar(id),
        (Func::Sysmis, Some((_, id))) if !is_string(ds, id) => Node::SysmisVar(id),
        (Func::Missing, _) => Node::MissingExpr(Box::new(bind(ds, arg)?)),
        _ => Node::SysmisExpr(Box::new(bind(ds, arg)?)),
    })
}

/// `a TO b`: every variable from `a` to `b` in dataset order (all must be numeric).
#[inline(never)]
fn expand_range(ds: &Dataset, func: Func, first: &Expr, last: &Expr, out: &mut Vec<Node>) -> Result<(), TransformError> {
    let (Expr::Var(a), Expr::Var(b)) = (first, last) else {
        return Err(TransformError::InvalidArguments("TO needs variable names".into()));
    };
    if !func.is_statistical() {
        return Err(TransformError::InvalidArguments(format!("TO is not allowed in arguments of {}", func.name())));
    }
    let (ia, ib) = (lookup(ds, a)?, lookup(ds, b)?);
    if ia.0 > ib.0 {
        return Err(TransformError::InvalidArguments(format!("In {a} TO {b}, {a} comes after {b} in the dataset")));
    }
    for i in ia.0..=ib.0 {
        let id = VarId(i);
        if is_string(ds, id) {
            return Err(TransformError::Type(format!(
                "String variable {} in range {a} TO {b} cannot be used in {}",
                ds.var(id).name,
                func.name()
            )));
        }
        out.push(Node::Var(id));
    }
    Ok(())
}

fn truth(v: bool) -> f64 {
    if v { 1.0 } else { 0.0 }
}

impl StrNode {
    fn get<'a>(&'a self, ds: &'a Dataset, row: usize) -> &'a str {
        let s = match self {
            StrNode::Lit(s) => s.as_str(),
            StrNode::Var(id) => match ds.column(*id) {
                Column::Text(v) => v[row].as_str(),
                Column::Numeric(_) => "",
            },
        };
        // SPSS pads strings with blanks: trailing spaces never matter.
        s.trim_end_matches(' ')
    }
}

impl Node {
    /// Value for case `row` (NaN = system-missing).
    pub(crate) fn eval(&self, ds: &Dataset, row: usize) -> f64 {
        match self {
            Node::Num(v) => *v,
            Node::Var(id) => {
                if ds.is_missing(*id, row) {
                    f64::NAN
                } else {
                    ds.numeric(*id)[row]
                }
            }
            Node::Neg(x) => -x.eval(ds, row),
            Node::Arith(op, l, r) => {
                let (a, b) = (l.eval(ds, row), r.eval(ds, row));
                if a.is_nan() || b.is_nan() {
                    // SPSS: 0 * missing = missing * 0 = 0 and 0 / missing = 0 (a valid exact zero).
                    let zero = match op {
                        Op::Mul => a == 0.0 || b == 0.0,
                        Op::Div => a == 0.0,
                        _ => false,
                    };
                    if zero {
                        return 0.0;
                    }
                    return f64::NAN;
                }
                finite(match op {
                    Op::Add => a + b,
                    Op::Sub => a - b,
                    Op::Mul => a * b,
                    Op::Div => a / b,
                    _ => a.powf(b),
                })
            }
            Node::Cmp(op, l, r) => {
                let (a, b) = (l.eval(ds, row), r.eval(ds, row));
                if a.is_nan() || b.is_nan() {
                    return f64::NAN;
                }
                truth(compare(*op, a.partial_cmp(&b)))
            }
            Node::StrCmp(op, l, r) => str_cmp(*op, l, r, ds, row),
            Node::And(l, r) => {
                let (a, b) = (l.eval(ds, row), r.eval(ds, row));
                if a == 0.0 || b == 0.0 {
                    0.0
                } else if a.is_nan() || b.is_nan() {
                    f64::NAN
                } else {
                    1.0
                }
            }
            Node::Or(l, r) => {
                let (a, b) = (l.eval(ds, row), r.eval(ds, row));
                let t = |v: f64| !v.is_nan() && v != 0.0;
                if t(a) || t(b) {
                    1.0
                } else if a.is_nan() || b.is_nan() {
                    f64::NAN
                } else {
                    0.0
                }
            }
            Node::Not(x) => {
                let a = x.eval(ds, row);
                if a.is_nan() { f64::NAN } else { truth(a == 0.0) }
            }
            Node::Scalar(f, args) => functions::scalar(*f, &eval_all(args, ds, row)),
            Node::Stat { func, min_valid, args } => functions::statistical(*func, &eval_all(args, ds, row), *min_valid),
            Node::Lag(id) => {
                if row == 0 {
                    f64::NAN
                } else {
                    Node::Var(*id).eval(ds, row - 1)
                }
            }
            Node::MissingVar(id) => truth(ds.is_missing(*id, row)),
            Node::MissingExpr(x) | Node::SysmisExpr(x) => truth(x.eval(ds, row).is_nan()),
            Node::SysmisVar(id) => truth(ds.numeric(*id)[row].is_nan()),
        }
    }
}

fn compare(op: Op, ord: Option<std::cmp::Ordering>) -> bool {
    use std::cmp::Ordering::*;
    let Some(o) = ord else { return false };
    match op {
        Op::Eq => o == Equal,
        Op::Ne => o != Equal,
        Op::Lt => o == Less,
        Op::Gt => o == Greater,
        Op::Le => o != Greater,
        _ => o != Less,
    }
}

/// Evaluate function arguments (kept out of `Node::eval` to keep its frame small).
#[inline(never)]
fn eval_all(args: &[Node], ds: &Dataset, row: usize) -> Vec<f64> {
    args.iter().map(|a| a.eval(ds, row)).collect()
}

#[inline(never)]
fn str_cmp(op: Op, l: &StrNode, r: &StrNode, ds: &Dataset, row: usize) -> f64 {
    truth(compare(op, Some(l.get(ds, row).cmp(r.get(ds, row)))))
}
