//! Transform → Compute Variable.

use crate::eval::{Node, bind};
use crate::functions::finite;
use crate::lexer::is_reserved;
use crate::parser::{ParseError, parse};
use dataset::{Column, Dataset, VarId, VarType, Variable, validate_name};
use std::fmt;

/// What the Compute Variable dialog submits.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ComputeSpec {
    /// Target variable (new or existing numeric variable).
    pub target: String,
    /// Numeric expression, e.g. `ABS(RES_1)`.
    pub expression: String,
    /// Optional `IF` condition: only cases where it is true are computed.
    pub condition: Option<String>,
    /// Variable label to set on the target (`None` keeps an existing label).
    pub label: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComputeReport {
    pub target: VarId,
    /// True when the target variable was newly created.
    pub created: bool,
    /// Number of cases where the target is system-missing afterwards.
    pub n_sysmis: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TransformError {
    /// Syntax error in the expression.
    Parse(ParseError),
    /// Syntax error in the IF condition.
    ConditionParse(ParseError),
    UnknownVariable(String),
    /// Type mismatch (string where a number is expected, etc.).
    Type(String),
    InvalidTarget(String),
    /// Wrong function arguments (count, `.n` suffix, TO range order).
    InvalidArguments(String),
}

impl fmt::Display for TransformError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TransformError::Parse(e) => write!(f, "Syntax error in expression: {e}"),
            TransformError::ConditionParse(e) => write!(f, "Syntax error in condition: {e}"),
            TransformError::UnknownVariable(n) => write!(f, "Unknown variable {n}"),
            TransformError::Type(m) | TransformError::InvalidTarget(m) | TransformError::InvalidArguments(m) => {
                f.write_str(m)
            }
        }
    }
}

impl std::error::Error for TransformError {}

/// Compute `spec.target = spec.expression` for every case (or only cases where
/// `spec.condition` is true). A new target is numeric (width 8, 2 decimals) and
/// system-missing where not computed; an existing numeric target keeps its metadata and
/// its values in cases not computed. Cases are evaluated in order, so `LAG(target)` sees
/// values computed for earlier cases. Nothing is changed when an error is returned.
pub fn compute(ds: &mut Dataset, spec: &ComputeSpec) -> Result<ComputeReport, TransformError> {
    let target = spec.target.trim();
    if validate_name(target).is_err() || is_reserved(target) {
        return Err(TransformError::InvalidTarget(format!("Invalid target variable name '{target}'")));
    }
    let existing = ds.find(target);
    if let Some(id) = existing
        && matches!(ds.var(id).var_type, VarType::String { .. })
    {
        return Err(TransformError::InvalidTarget(format!(
            "Target variable {} is a string variable; only numeric targets are supported",
            ds.var(id).name
        )));
    }

    let expr = parse(&spec.expression).map_err(TransformError::Parse)?;
    let node = bind(ds, &expr)?;
    let cond: Option<Node> = match spec.condition.as_deref().map(str::trim) {
        Some(c) if !c.is_empty() => Some(bind(ds, &parse(c).map_err(TransformError::ConditionParse)?)?),
        _ => None,
    };

    let id = match existing {
        Some(id) => id,
        None => {
            let col = Column::Numeric(vec![f64::NAN; ds.n_cases()]);
            ds.add_variable(Variable::numeric(target), col)
                .map_err(|e| TransformError::InvalidTarget(e.to_string()))?
        }
    };
    if let Some(label) = &spec.label {
        ds.var_mut(id).label = (!label.trim().is_empty()).then(|| label.clone());
    }

    for row in 0..ds.n_cases() {
        if let Some(c) = &cond {
            let v = c.eval(ds, row);
            if v.is_nan() || v == 0.0 {
                continue;
            }
        }
        let v = finite(node.eval(ds, row));
        ds.set_numeric(id, row, v).map_err(|e| TransformError::InvalidTarget(e.to_string()))?;
    }
    let n_sysmis = ds.numeric(id).iter().filter(|v| v.is_nan()).count();
    Ok(ComputeReport { target: id, created: existing.is_none(), n_sysmis })
}
