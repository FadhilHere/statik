//! Data model: variables, missing values, CSV loading.
mod csv_load;
mod dataset;
mod variable;

pub use csv_load::load_csv;
pub use dataset::{
    is_reserved_word, truncate_bytes, validate_name, Column, Dataset, DatasetError, VarId, MAX_NAME_BYTES, RESERVED_WORDS,
};
pub use variable::{Measure, MissingSpec, OrderedValue, VarType, Variable};
