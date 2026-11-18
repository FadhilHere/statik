//! Data model: variables, missing values, CSV loading.
mod csv_load;
mod dataset;
mod variable;

pub use csv_load::load_csv;
pub use dataset::{validate_name, Column, Dataset, DatasetError, VarId};
pub use variable::{Measure, MissingSpec, OrderedValue, VarType, Variable};
