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
    #[error("invalid string width {0} (must be 1 to 32767 bytes)")]
    InvalidWidth(usize),
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
    /// Name of the weight variable (as in a `.sav` header). Statik's analyses ignore it.
    weight: Option<String>,
    /// File label (`.sav` header).
    file_label: Option<String>,
    /// Document lines (`.sav` record 6).
    documents: Vec<String>,
}

/// Longest variable name SPSS accepts, in UTF-8 bytes.
pub const MAX_NAME_BYTES: usize = 64;

/// SPSS reserved words, which are never valid variable names.
pub const RESERVED_WORDS: [&str; 13] = ["ALL", "AND", "BY", "EQ", "GE", "GT", "LE", "LT", "NE", "NOT", "OR", "TO", "WITH"];

/// `name` is an SPSS reserved word (case-insensitive).
pub fn is_reserved_word(name: &str) -> bool {
    RESERVED_WORDS.iter().any(|w| w.eq_ignore_ascii_case(name))
}

/// SPSS-style: starts with a letter, then letters/digits/`_`/`.`/`$`/`#`/`@`, at most 64 bytes
/// (UTF-8), and not a reserved word (`ALL`, `AND`, `BY`, ...).
pub fn validate_name(name: &str) -> Result<(), DatasetError> {
    let bad = || Err(DatasetError::InvalidName(name.to_string()));
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_alphabetic() => {}
        _ => return bad(),
    }
    if name.len() > MAX_NAME_BYTES || is_reserved_word(name) {
        return bad();
    }
    if chars.all(|c| c.is_alphanumeric() || matches!(c, '_' | '.' | '$' | '#' | '@')) { Ok(()) } else { bad() }
}

/// Longest prefix of `s` that is at most `max` bytes, cut on a character boundary.
pub fn truncate_bytes(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    let mut end = max;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

const MAX_STRING_WIDTH: usize = 32767;

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
        if self.weight() == Some(id) {
            self.weight = None;
        }
        self.vars.remove(id.0);
        self.cols.remove(id.0);
        Ok(())
    }

    /// The weight variable kept from a `.sav` file, if any. Statik's analyses are unweighted.
    pub fn weight(&self) -> Option<VarId> {
        let id = self.find(self.weight.as_deref()?)?;
        matches!(self.vars[id.0].var_type, VarType::Numeric).then_some(id)
    }

    /// Set or clear the weight variable; it must be numeric.
    pub fn set_weight(&mut self, id: Option<VarId>) -> Result<(), DatasetError> {
        self.weight = match id {
            None => None,
            Some(id) => {
                let v = self.vars.get(id.0).ok_or(DatasetError::OutOfRange(id.0))?;
                if !matches!(v.var_type, VarType::Numeric) {
                    return Err(DatasetError::TypeMismatch);
                }
                Some(v.name.clone())
            }
        };
        Ok(())
    }

    pub fn file_label(&self) -> Option<&str> {
        self.file_label.as_deref()
    }

    pub fn set_file_label(&mut self, label: Option<String>) {
        self.file_label = label;
    }

    /// Document lines (SPSS "DOCUMENT" text).
    pub fn documents(&self) -> &[String] {
        &self.documents
    }

    pub fn set_documents(&mut self, lines: Vec<String>) {
        self.documents = lines;
    }

    /// Change the storage width (bytes) of a string variable. Values longer than `width` are cut
    /// on a character boundary; returns how many were cut. The display width is not changed.
    pub fn set_string_width(&mut self, id: VarId, width: usize) -> Result<usize, DatasetError> {
        let var = self.vars.get_mut(id.0).ok_or(DatasetError::OutOfRange(id.0))?;
        let Column::Text(vals) = &mut self.cols[id.0] else {
            return Err(DatasetError::TypeMismatch);
        };
        if width == 0 || width > MAX_STRING_WIDTH {
            return Err(DatasetError::InvalidWidth(width));
        }
        let mut cut = 0;
        for s in vals.iter_mut() {
            if s.len() > width {
                let keep = truncate_bytes(s, width).len();
                s.truncate(keep);
                cut += 1;
            }
        }
        var.var_type = VarType::String { width };
        Ok(cut)
    }

    pub fn rename_variable(&mut self, id: VarId, new_name: &str) -> Result<(), DatasetError> {
        if id.0 >= self.vars.len() {
            return Err(DatasetError::OutOfRange(id.0));
        }
        validate_name(new_name)?;
        if self.find(new_name).is_some_and(|other| other != id) {
            return Err(DatasetError::DuplicateName(new_name.to_string()));
        }
        if self.weight() == Some(id) {
            self.weight = Some(new_name.to_string());
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

    /// Convert a variable's type, converting values, and return how many values were lost.
    ///
    /// - Numeric -> String formats each value with the variable's decimals ("" for NaN). The
    ///   storage width is the requested one, raised to the longest value (and at least 8) so no
    ///   text is cut. Returns 0.
    /// - String -> Numeric parses the trimmed text (failure / non-finite -> NaN). Returns the
    ///   number of non-blank cells that could not be parsed and became system-missing.
    /// - String -> String changes the storage width like [`Dataset::set_string_width`] and
    ///   returns the number of values cut.
    ///
    /// On a type change value labels, missing specs and the kept `.sav` format are reset (and
    /// the weight is cleared if this was the weight variable); Numeric -> Numeric is a no-op.
    pub fn set_type(&mut self, id: VarId, t: VarType) -> Result<usize, DatasetError> {
        let Some(var) = self.vars.get(id.0) else {
            return Err(DatasetError::OutOfRange(id.0));
        };
        let was_weight = self.weight() == Some(id);
        let lost = match (&self.cols[id.0], t) {
            (Column::Numeric(v), VarType::String { width: asked }) => {
                let dec = var.decimals;
                let text: Vec<String> = v
                    .iter()
                    .map(|&x| if x.is_nan() { String::new() } else { format!("{:.*}", dec, x + 0.0) })
                    .collect();
                let longest = text.iter().map(String::len).max().unwrap_or(0).max(8);
                let mut nv = Variable::string(&var.name, asked.max(longest).min(MAX_STRING_WIDTH));
                nv.label = var.label.clone();
                self.vars[id.0] = nv;
                self.cols[id.0] = Column::Text(text);
                0
            }
            (Column::Text(v), VarType::Numeric) => {
                let mut lost = 0;
                let nums: Vec<f64> = v
                    .iter()
                    .map(|s| match s.trim().parse::<f64>().ok().filter(|x| x.is_finite()) {
                        Some(x) => x + 0.0,
                        None => {
                            lost += usize::from(!s.trim().is_empty());
                            f64::NAN
                        }
                    })
                    .collect();
                let mut nv = Variable::numeric(&var.name);
                nv.label = var.label.clone();
                self.vars[id.0] = nv;
                self.cols[id.0] = Column::Numeric(nums);
                lost
            }
            (Column::Text(_), VarType::String { width }) => return self.set_string_width(id, width),
            (Column::Numeric(_), VarType::Numeric) => return Ok(0),
        };
        if was_weight {
            self.weight = None;
        }
        Ok(lost)
    }

    /// `prefix` + smallest n >= 1 such that the name is unused.
    pub fn unique_name(&self, prefix: &str) -> String {
        (1..).map(|n| format!("{prefix}{n}")).find(|c| self.find(c).is_none()).unwrap()
    }
}
