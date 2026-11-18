use std::cmp::Ordering;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Measure {
    Scale,
    Ordinal,
    Nominal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VarType {
    Numeric,
    String { width: usize },
}

#[derive(Clone, Debug, PartialEq)]
pub enum MissingSpec {
    None,
    Discrete(Vec<f64>),
    Range { lo: f64, hi: f64, discrete: Option<f64> },
}

impl MissingSpec {
    pub fn matches(&self, v: f64) -> bool {
        match self {
            MissingSpec::None => false,
            MissingSpec::Discrete(l) => l.contains(&v),
            MissingSpec::Range { lo, hi, discrete } => {
                (v >= *lo && v <= *hi) || discrete.is_some_and(|d| d == v)
            }
        }
    }
}

/// f64 newtype ordered by `total_cmp`, usable as a map key. -0.0 and 0.0 are the same value.
#[derive(Clone, Copy, Debug)]
pub struct OrderedValue(pub f64);

impl OrderedValue {
    /// Key with -0.0 normalised to 0.0 (so a stored key never displays as "-0").
    pub fn new(v: f64) -> Self {
        OrderedValue(v + 0.0)
    }
}

impl PartialEq for OrderedValue {
    fn eq(&self, o: &Self) -> bool {
        self.cmp(o) == Ordering::Equal
    }
}
impl Eq for OrderedValue {}
impl PartialOrd for OrderedValue {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for OrderedValue {
    fn cmp(&self, o: &Self) -> Ordering {
        // x + 0.0 maps -0.0 to 0.0 and leaves every other value unchanged.
        (self.0 + 0.0).total_cmp(&(o.0 + 0.0))
    }
}

#[derive(Clone, Debug)]
pub struct Variable {
    pub name: String,
    pub label: Option<String>,
    pub var_type: VarType,
    pub width: usize,
    pub decimals: usize,
    pub value_labels: BTreeMap<OrderedValue, String>,
    pub missing: MissingSpec,
    pub measure: Measure,
}

impl Variable {
    /// Numeric scale variable with SPSS-like defaults (width 8, 2 decimals).
    pub fn numeric(name: &str) -> Self {
        Variable {
            name: name.to_string(),
            label: None,
            var_type: VarType::Numeric,
            width: 8,
            decimals: 2,
            value_labels: BTreeMap::new(),
            missing: MissingSpec::None,
            measure: Measure::Scale,
        }
    }

    pub fn string(name: &str, width: usize) -> Self {
        Variable {
            var_type: VarType::String { width },
            width,
            decimals: 0,
            measure: Measure::Nominal,
            ..Self::numeric(name)
        }
    }
}
