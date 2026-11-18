//! Reading and writing SPSS system files (`.sav`).
//!
//! Record layouts follow the GNU PSPP manual, "System File Format". Files are read in
//! either byte order, uncompressed or bytecode-compressed; `.zsav` (zlib) is not supported.
//! CSV and XLSX files can be imported too ([`import_csv`], [`import_xlsx`]); [`open_any`] picks by extension.
//! Files are written little-endian, bytecode-compressed and UTF-8 encoded.
mod compress;
mod csv_import;
mod names;
mod read;
mod records;
mod write;
mod xlsx_import;

pub use csv_import::{CsvOptions, CsvPreview, import_csv, sniff_csv};
pub use names::sanitize_name;
pub use read::read_sav;
pub use write::write_sav;
pub use xlsx_import::{import_xlsx, xlsx_sheet_names};

#[derive(Debug, thiserror::Error)]
pub enum SavError {
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid .sav file: {0}")]
    Format(String),
    #[error("unsupported .sav feature: {0}")]
    Unsupported(String),
    #[error("Invalid CSV: {0}")]
    Csv(String),
    #[error("Invalid XLSX: {0}")]
    Xlsx(String),
    #[error("Unsupported file type: {0}")]
    UnknownType(String),
    /// The dataset is internally inconsistent (a bug, not a problem with any file).
    #[error("Cannot save: {0}")]
    Internal(String),
}

/// What a reader could not represent exactly: user-facing English sentences, one per issue
/// (e.g. an ignored weight variable or a date format shown as numbers). Empty when nothing
/// needs saying.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReadReport {
    pub notes: Vec<String>,
}

/// Open a data file, choosing the reader by extension: `.sav`, `.csv`/`.txt`, `.xlsx`/`.xlsm`/`.xls`/`.ods`.
/// CSV and spreadsheet imports return an empty report.
pub fn open_any(path: &std::path::Path) -> Result<(dataset::Dataset, ReadReport), SavError> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "sav" => read_sav(path),
        "csv" | "txt" => Ok((import_csv(path, &CsvOptions::default())?, ReadReport::default())),
        "xlsx" | "xlsm" | "xls" | "ods" => Ok((import_xlsx(path, None, true)?, ReadReport::default())),
        other => Err(SavError::UnknownType(format!(".{other}"))),
    }
}
