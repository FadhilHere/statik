//! Transform → Recode into Different Variables.

use crate::compute::TransformError;
use crate::lexer::is_reserved;
use dataset::{Column, Dataset, VarId, VarType, Variable, validate_name};

/// Left-hand side of a recode rule.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OldValue {
    Value(f64),
    /// Inclusive range `lo THRU hi`.
    Range(f64, f64),
    /// `LOWEST THRU v`.
    Lowest(f64),
    /// `v THRU HIGHEST`.
    Highest(f64),
    /// System-missing (NaN) only.
    SysMis,
    /// System-missing or user-missing.
    Missing,
    /// Anything not matched by an earlier rule (including system-missing).
    Else,
}

/// Right-hand side of a recode rule.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NewValue {
    Value(f64),
    SysMis,
    /// Keep the old value.
    Copy,
}

/// What the Recode into Different Variables dialog submits.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RecodeSpec {
    /// (input variable, output name, optional output label).
    pub pairs: Vec<(VarId, String, Option<String>)>,
    /// Rules in order; the first matching rule wins. Unmatched values become system-missing.
    pub rules: Vec<(OldValue, NewValue)>,
}

fn matches(old: &OldValue, raw: f64, user_missing: bool) -> bool {
    match *old {
        OldValue::Value(v) => !raw.is_nan() && raw == v,
        OldValue::Range(lo, hi) => !raw.is_nan() && raw >= lo && raw <= hi,
        OldValue::Lowest(v) => !raw.is_nan() && raw <= v,
        OldValue::Highest(v) => !raw.is_nan() && raw >= v,
        OldValue::SysMis => raw.is_nan(),
        OldValue::Missing => raw.is_nan() || user_missing,
        OldValue::Else => true,
    }
}

/// Create one new numeric variable per pair in `spec.pairs`. Rules are checked in order
/// against the raw value; user-missing values are matched by value rules like any other.
/// Nothing is changed when an error is returned.
pub fn recode(ds: &mut Dataset, spec: &RecodeSpec) -> Result<Vec<VarId>, TransformError> {
    if spec.rules.is_empty() {
        return Err(TransformError::InvalidArguments("Recode needs at least one rule".into()));
    }
    for (old, _) in &spec.rules {
        let bounds: &[f64] = match old {
            OldValue::Value(v) | OldValue::Lowest(v) | OldValue::Highest(v) => &[*v],
            OldValue::Range(lo, hi) => &[*lo, *hi],
            _ => &[],
        };
        if bounds.iter().any(|b| b.is_nan()) {
            return Err(TransformError::InvalidArguments("A recode value must be a number".into()));
        }
        if let OldValue::Range(lo, hi) = old
            && lo > hi
        {
            return Err(TransformError::InvalidArguments(format!(
                "Range {lo} THRU {hi}: the lower bound is greater than the upper bound"
            )));
        }
    }
    let mut names: Vec<String> = Vec::new();
    for (id, out, _) in &spec.pairs {
        let out = out.trim();
        if validate_name(out).is_err() || is_reserved(out) {
            return Err(TransformError::InvalidTarget(format!("Invalid target variable name '{out}'")));
        }
        // Same comparison as `Dataset::find` (full Unicode lower case).
        let lower = out.to_lowercase();
        if ds.find(out).is_some() || names.iter().any(|n| n.to_lowercase() == lower) {
            return Err(TransformError::NameTaken(out.to_string()));
        }
        names.push(out.to_string());
        let var = ds.try_var(*id).ok_or_else(|| TransformError::UnknownVariable(format!("#{}", id.0)))?;
        if !matches!(var.var_type, VarType::Numeric) {
            return Err(TransformError::NotNumeric(var.name.clone()));
        }
    }

    let mut results = Vec::new();
    for (id, _, _) in &spec.pairs {
        let var = ds.var(*id);
        let vals: Vec<f64> = ds
            .numeric(*id)
            .iter()
            .map(|&raw| {
                let um = !raw.is_nan() && var.missing.matches(raw);
                let out = spec
                    .rules
                    .iter()
                    .find(|(old, _)| matches(old, raw, um))
                    .map_or(f64::NAN, |(_, new)| match new {
                        NewValue::Value(v) => *v,
                        NewValue::SysMis => f64::NAN,
                        NewValue::Copy => raw,
                    });
                out + 0.0
            })
            .collect();
        results.push(vals);
    }

    let mut ids = Vec::new();
    for (((_, _, label), name), vals) in spec.pairs.iter().zip(names).zip(results) {
        let mut v = Variable::numeric(&name);
        v.decimals = if vals.iter().all(|x| x.is_nan() || x.fract() == 0.0) { 0 } else { 2 };
        v.label = label.clone().filter(|l| !l.trim().is_empty());
        match ds.add_variable(v, Column::Numeric(vals)) {
            Ok(id) => ids.push(id),
            Err(e) => {
                // Should not happen after the checks above; undo so nothing is changed.
                for id in ids.into_iter().rev() {
                    let _ = ds.delete_variable(id);
                }
                return Err(TransformError::InvalidTarget(e.to_string()));
            }
        }
    }
    Ok(ids)
}
