//! Reading and writing SPSS system files (`.sav`).
//!
//! Record layouts follow the GNU PSPP manual, "System File Format". Files are read in
//! either byte order, uncompressed or bytecode-compressed; `.zsav` (zlib) is not supported.
//! Files are written little-endian, bytecode-compressed and UTF-8 encoded.
mod compress;
mod read;
mod records;
mod write;

pub use read::read_sav;
pub use write::write_sav;

#[derive(Debug, thiserror::Error)]
pub enum SavError {
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid .sav file: {0}")]
    Format(String),
    #[error("unsupported .sav feature: {0}")]
    Unsupported(String),
}
