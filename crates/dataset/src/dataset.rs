use crate::variable::*;

#[derive(Debug, thiserror::Error)]
pub enum DatasetError {
    #[error("invalid variable name: {0}")]
    InvalidName(String),
    #[error("duplicate variable name: {0}")]
    DuplicateName(String),
    #[error("column length {got} does not match case count {expected}")]
    LengthMismatch { expected: usize, got: usize },
    #[error("column type does not match variable type")]
    TypeMismatch,
    #[error("csv error: {0}")]
    Csv(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Clone, Debug)]
pub enum Column {
    Numeric(Vec<f64>),
    Text(Vec<String>),
}

impl Column {
    pub fn len(&self) -> usize {
        match self {
            Column::Numeric(v) => v.len(),
            Column::Text(v) => v.len(),
        }
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct VarId(pub usize);

#[derive(Clone, Debug, Default)]
pub struct Dataset {
    vars: Vec<Variable>,
    cols: Vec<Column>,
}

/// SPSS-style: starts with a letter, then letters/digits/`_`/`.`/`$`/`#`/`@`, max 64 chars.
pub fn validate_name(name: &str) -> Result<(), DatasetError> {
    let bad = || Err(DatasetError::InvalidName(name.to_string()));
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_alphabetic() => {}
        _ => return bad(),
    }
    if name.chars().count() > 64 {
        return bad();
    }
    if chars.all(|c| c.is_alphanumeric() || matches!(c, '_' | '.' | '$' | '#' | '@')) {
        Ok(())
    } else {
        bad()
    }
}

impl Dataset {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn n_cases(&self) -> usize {
        self.cols.first().map_or(0, Column::len)
    }

    pub fn n_vars(&self) -> usize {
        self.vars.len()
    }

    pub fn add_variable(&mut self, v: Variable, col: Column) -> Result<VarId, DatasetError> {
        validate_name(&v.name)?;
        if self.find(&v.name).is_some() {
            return Err(DatasetError::DuplicateName(v.name));
        }
        match (&v.var_type, &col) {
            (VarType::Numeric, Column::Numeric(_)) | (VarType::String { .. }, Column::Text(_)) => {}
            _ => return Err(DatasetError::TypeMismatch),
        }
        if !self.vars.is_empty() && col.len() != self.n_cases() {
            return Err(DatasetError::LengthMismatch { expected: self.n_cases(), got: col.len() });
        }
        self.vars.push(v);
        self.cols.push(col);
        Ok(VarId(self.vars.len() - 1))
    }

    pub fn var(&self, id: VarId) -> &Variable {
        &self.vars[id.0]
    }

    pub fn var_mut(&mut self, id: VarId) -> &mut Variable {
        &mut self.vars[id.0]
    }

    pub fn column(&self, id: VarId) -> &Column {
        &self.cols[id.0]
    }

    /// Case-insensitive lookup.
    pub fn find(&self, name: &str) -> Option<VarId> {
        let lower = name.to_lowercase();
        self.vars.iter().position(|v| v.name.to_lowercase() == lower).map(VarId)
    }

    /// Panics if the variable is not numeric.
    pub fn numeric(&self, id: VarId) -> &[f64] {
        match &self.cols[id.0] {
            Column::Numeric(v) => v,
            Column::Text(_) => panic!("variable '{}' is not numeric", self.vars[id.0].name),
        }
    }

    /// NaN or user-missing. String variables are never missing.
    pub fn is_missing(&self, id: VarId, row: usize) -> bool {
        match &self.cols[id.0] {
            Column::Numeric(v) => v[row].is_nan() || self.vars[id.0].missing.matches(v[row]),
            Column::Text(_) => false,
        }
    }

    /// Rows with no missing value in any of `ids`.
    pub fn listwise_rows(&self, ids: &[VarId]) -> Vec<usize> {
        (0..self.n_cases()).filter(|&r| ids.iter().all(|&id| !self.is_missing(id, r))).collect()
    }

    /// `prefix` + smallest n >= 1 such that the name is unused.
    pub fn unique_name(&self, prefix: &str) -> String {
        (1..).map(|n| format!("{prefix}{n}")).find(|c| self.find(c).is_none()).unwrap()
    }
}
