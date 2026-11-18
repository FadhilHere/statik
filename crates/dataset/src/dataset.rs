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
    #[error("index out of range: {0}")]
    OutOfRange(usize),
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
    if chars.all(|c| c.is_alphanumeric() || matches!(c, '_' | '.' | '$' | '#' | '@')) { Ok(()) } else { bad() }
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

    /// Checked lookup: `None` when `id` is stale / out of range.
    pub fn try_var(&self, id: VarId) -> Option<&Variable> {
        self.vars.get(id.0)
    }

    /// Insert a variable at position `at` (`at <= n_vars`). Later variables shift one
    /// position right, so their positional `VarId`s change.
    pub fn insert_variable(&mut self, at: usize, v: Variable, col: Column) -> Result<VarId, DatasetError> {
        if at > self.vars.len() {
            return Err(DatasetError::OutOfRange(at));
        }
        self.add_variable(v, col)?; // validates, appends last
        let v = self.vars.pop().expect("just pushed");
        let c = self.cols.pop().expect("just pushed");
        self.vars.insert(at, v);
        self.cols.insert(at, c);
        Ok(VarId(at))
    }

    /// Remove a variable and its column. Because `VarId` is a positional index, every
    /// variable after `id` shifts one position left (its `VarId` decreases by one).
    pub fn delete_variable(&mut self, id: VarId) -> Result<(), DatasetError> {
        if id.0 >= self.vars.len() {
            return Err(DatasetError::OutOfRange(id.0));
        }
        self.vars.remove(id.0);
        self.cols.remove(id.0);
        Ok(())
    }

    pub fn rename_variable(&mut self, id: VarId, new_name: &str) -> Result<(), DatasetError> {
        if id.0 >= self.vars.len() {
            return Err(DatasetError::OutOfRange(id.0));
        }
        validate_name(new_name)?;
        if self.find(new_name).is_some_and(|other| other != id) {
            return Err(DatasetError::DuplicateName(new_name.to_string()));
        }
        self.vars[id.0].name = new_name.to_string();
        Ok(())
    }

    /// Insert a blank case (NaN / "") before row `at` (clamped to the end).
    pub fn insert_case(&mut self, at: usize) {
        let at = at.min(self.n_cases());
        for c in &mut self.cols {
            match c {
                Column::Numeric(v) => v.insert(at, f64::NAN),
                Column::Text(v) => v.insert(at, String::new()),
            }
        }
    }

    pub fn delete_case(&mut self, row: usize) -> Result<(), DatasetError> {
        if row >= self.n_cases() {
            return Err(DatasetError::OutOfRange(row));
        }
        for c in &mut self.cols {
            match c {
                Column::Numeric(v) => {
                    v.remove(row);
                }
                Column::Text(v) => {
                    v.remove(row);
                }
            }
        }
        Ok(())
    }

    /// Set a numeric cell (-0.0 is stored as 0.0).
    pub fn set_numeric(&mut self, id: VarId, row: usize, v: f64) -> Result<(), DatasetError> {
        match self.cols.get_mut(id.0) {
            None => Err(DatasetError::OutOfRange(id.0)),
            Some(Column::Text(_)) => Err(DatasetError::TypeMismatch),
            Some(Column::Numeric(c)) => match c.get_mut(row) {
                None => Err(DatasetError::OutOfRange(row)),
                Some(cell) => {
                    *cell = v + 0.0;
                    Ok(())
                }
            },
        }
    }

    pub fn set_text(&mut self, id: VarId, row: usize, s: &str) -> Result<(), DatasetError> {
        match self.cols.get_mut(id.0) {
            None => Err(DatasetError::OutOfRange(id.0)),
            Some(Column::Numeric(_)) => Err(DatasetError::TypeMismatch),
            Some(Column::Text(c)) => match c.get_mut(row) {
                None => Err(DatasetError::OutOfRange(row)),
                Some(cell) => {
                    *cell = s.to_string();
                    Ok(())
                }
            },
        }
    }

    /// Convert a variable's type, converting values. Numeric -> String formats each value
    /// with the variable's decimals ("" for NaN; width = max(8, longest)). String -> Numeric
    /// parses the trimmed text (failure / non-finite -> NaN). Value labels and missing
    /// specs are reset; a same-type call is a no-op.
    pub fn set_type(&mut self, id: VarId, t: VarType) -> Result<(), DatasetError> {
        let Some(var) = self.vars.get(id.0) else {
            return Err(DatasetError::OutOfRange(id.0));
        };
        match (&self.cols[id.0], t) {
            (Column::Numeric(v), VarType::String { .. }) => {
                let dec = var.decimals;
                let text: Vec<String> = v
                    .iter()
                    .map(|&x| if x.is_nan() { String::new() } else { format!("{:.*}", dec, x + 0.0) })
                    .collect();
                let width = text.iter().map(|s| s.chars().count()).max().unwrap_or(0).max(8);
                let mut nv = Variable::string(&var.name, width);
                nv.label = var.label.clone();
                self.vars[id.0] = nv;
                self.cols[id.0] = Column::Text(text);
            }
            (Column::Text(v), VarType::Numeric) => {
                let nums: Vec<f64> = v
                    .iter()
                    .map(|s| s.trim().parse::<f64>().ok().filter(|x| x.is_finite()).map_or(f64::NAN, |x| x + 0.0))
                    .collect();
                let mut nv = Variable::numeric(&var.name);
                nv.label = var.label.clone();
                self.vars[id.0] = nv;
                self.cols[id.0] = Column::Numeric(nums);
            }
            _ => {}
        }
        Ok(())
    }

    /// `prefix` + smallest n >= 1 such that the name is unused.
    pub fn unique_name(&self, prefix: &str) -> String {
        (1..).map(|n| format!("{prefix}{n}")).find(|c| self.find(c).is_none()).unwrap()
    }
}
