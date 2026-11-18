//! Built-in functions: name table, arity, and numeric implementations.

/// Every built-in function.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Func {
    Abs,
    Sqrt,
    Ln,
    Lg10,
    Exp,
    Rnd,
    Trunc,
    Mod,
    Mean,
    Sum,
    Sd,
    Variance,
    Min,
    Max,
    Nvalid,
    Lag,
    Missing,
    Sysmis,
}

const ALL: [Func; 18] = [
    Func::Abs,
    Func::Sqrt,
    Func::Ln,
    Func::Lg10,
    Func::Exp,
    Func::Rnd,
    Func::Trunc,
    Func::Mod,
    Func::Mean,
    Func::Sum,
    Func::Sd,
    Func::Variance,
    Func::Min,
    Func::Max,
    Func::Nvalid,
    Func::Lag,
    Func::Missing,
    Func::Sysmis,
];

impl Func {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Func::Abs => "ABS",
            Func::Sqrt => "SQRT",
            Func::Ln => "LN",
            Func::Lg10 => "LG10",
            Func::Exp => "EXP",
            Func::Rnd => "RND",
            Func::Trunc => "TRUNC",
            Func::Mod => "MOD",
            Func::Mean => "MEAN",
            Func::Sum => "SUM",
            Func::Sd => "SD",
            Func::Variance => "VARIANCE",
            Func::Min => "MIN",
            Func::Max => "MAX",
            Func::Nvalid => "NVALID",
            Func::Lag => "LAG",
            Func::Missing => "MISSING",
            Func::Sysmis => "SYSMIS",
        }
    }

    /// (minimum, maximum) number of arguments; `None` = unlimited.
    pub(crate) fn arity(self) -> (usize, Option<usize>) {
        match self {
            Func::Rnd | Func::Trunc => (1, Some(2)),
            Func::Mod => (2, Some(2)),
            f if f.is_statistical() => (1, None),
            _ => (1, Some(1)),
        }
    }

    /// Variadic functions over valid values; they accept `a TO b` ranges.
    pub(crate) fn is_statistical(self) -> bool {
        matches!(self, Func::Mean | Func::Sum | Func::Sd | Func::Variance | Func::Min | Func::Max | Func::Nvalid)
    }

    /// Whether the `.n` suffix (minimum number of valid arguments) is allowed.
    fn accepts_suffix(self) -> bool {
        self.is_statistical() && self != Func::Nvalid
    }

    /// Minimum number of valid values when no `.n` suffix is given.
    pub(crate) fn default_min_valid(self) -> usize {
        match self {
            Func::Sd | Func::Variance => 2,
            Func::Nvalid => 0,
            _ => 1,
        }
    }
}

/// A resolved function name such as `MEAN.2`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct FuncRef {
    pub func: Func,
    /// Explicit `.n` suffix.
    pub min_valid: Option<usize>,
}

/// Resolve a (case-insensitive) function name, including an optional `.n` suffix.
pub(crate) fn resolve(name: &str) -> Result<FuncRef, String> {
    let upper = name.to_uppercase();
    let find = |n: &str| ALL.into_iter().find(|f| f.name() == n);
    if let Some(func) = find(&upper) {
        return Ok(FuncRef { func, min_valid: None });
    }
    if let Some((base, suffix)) = upper.rsplit_once('.')
        && let Some(func) = find(base)
        && !suffix.is_empty()
        && suffix.bytes().all(|b| b.is_ascii_digit())
    {
        if !func.accepts_suffix() {
            return Err(format!("Function {base} does not accept a .n suffix"));
        }
        return match suffix.parse::<usize>() {
            Ok(n) if n >= 1 => Ok(FuncRef { func, min_valid: Some(n) }),
            _ => Err(format!("Invalid minimum count in {upper}: must be a whole number of at least 1")),
        };
    }
    Err(format!("Unknown function {upper}"))
}

/// SPSS default fuzz for RND/TRUNC: 6 least-significant bits of a double.
const FUZZ: f64 = 1.0 / (1u64 << 47) as f64;

/// Round `x` to a multiple of `mult`, halves away from zero (with SPSS fuzz).
fn rnd(x: f64, mult: f64) -> f64 {
    if mult == 0.0 {
        return f64::NAN;
    }
    let y = x / mult;
    (y.abs() * (1.0 + FUZZ)).round().copysign(y) * mult
}

/// Truncate `x` toward zero to a multiple of `mult` (with SPSS fuzz).
fn trunc(x: f64, mult: f64) -> f64 {
    if mult == 0.0 {
        return f64::NAN;
    }
    let y = x / mult;
    (y.abs() * (1.0 + FUZZ)).trunc().copysign(y) * mult
}

/// Scalar functions (ABS .. MOD). Any missing (NaN) argument gives NaN; so do domain errors.
pub(crate) fn scalar(func: Func, args: &[f64]) -> f64 {
    if args.iter().any(|a| a.is_nan()) {
        return f64::NAN;
    }
    let a = args.first().copied().unwrap_or(f64::NAN);
    let b = args.get(1).copied();
    let r = match func {
        Func::Abs => a.abs(),
        Func::Sqrt if a >= 0.0 => a.sqrt(),
        Func::Ln if a > 0.0 => a.ln(),
        Func::Lg10 if a > 0.0 => a.log10(),
        Func::Exp => a.exp(),
        Func::Rnd => rnd(a, b.unwrap_or(1.0)),
        Func::Trunc => trunc(a, b.unwrap_or(1.0)),
        Func::Mod => match b {
            Some(b) if b != 0.0 => a % b,
            _ => f64::NAN,
        },
        _ => f64::NAN,
    };
    finite(r)
}

/// Statistical functions over the valid (non-missing) values in `vals`.
/// Returns NaN when fewer than `min_valid` values are valid (NVALID never is missing).
pub(crate) fn statistical(func: Func, vals: &[f64], min_valid: usize) -> f64 {
    let valid: Vec<f64> = vals.iter().copied().filter(|v| !v.is_nan()).collect();
    let n = valid.len();
    if func == Func::Nvalid {
        return n as f64;
    }
    if n == 0 || n < min_valid {
        return f64::NAN;
    }
    let sum: f64 = valid.iter().sum();
    let mean = sum / n as f64;
    let variance = || {
        if n < 2 {
            f64::NAN
        } else {
            valid.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1) as f64
        }
    };
    let r = match func {
        Func::Sum => sum,
        Func::Mean => mean,
        Func::Variance => variance(),
        Func::Sd => variance().sqrt(),
        Func::Min => valid.iter().copied().fold(f64::INFINITY, f64::min),
        Func::Max => valid.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        _ => f64::NAN,
    };
    finite(r)
}

/// Non-finite → NaN (system-missing); -0.0 → 0.0.
pub(crate) fn finite(v: f64) -> f64 {
    if v.is_finite() { v + 0.0 } else { f64::NAN }
}
