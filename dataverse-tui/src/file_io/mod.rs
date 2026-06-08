//! File I/O utilities for reading and writing tabular data files.
//!
//! Supports CSV and Excel formats for both export (Record -> file) and
//! import (file -> parsed rows) operations.

mod convert;
mod csv;
mod excel;

pub use convert::{ConvertError, string_to_value};
pub use csv::{read_csv, write_csv};
pub use excel::{ExcelSheet, list_sheets, read_excel, write_excel, write_excel_multi};

use std::path::Path;

/// Error type for file I/O operations.
#[derive(Debug, thiserror::Error)]
pub enum FileIoError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("CSV error: {0}")]
    Csv(#[from] ::csv::Error),

    #[error("Excel write error: {0}")]
    ExcelWrite(#[from] rust_xlsxwriter::XlsxError),

    #[error("Excel read error: {0}")]
    ExcelRead(#[from] calamine::Error),

    #[error("Sheet not found: {0}")]
    SheetNotFound(String),

    #[error("No sheets in workbook")]
    NoSheets,

    #[error("Empty file")]
    EmptyFile,

    #[error("Invalid sheet name '{name}': {reason}")]
    InvalidSheetName { name: String, reason: String },

    #[error("Operation cancelled")]
    Cancelled,
}

impl FileIoError {
    /// Returns `true` if this is a cancellation error.
    pub fn is_cancelled(&self) -> bool {
        matches!(self, Self::Cancelled)
    }
}

/// A parsed cell from a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CellValue {
    /// Blank/missing cell — leave unchanged.
    Empty,
    /// Explicit `null` marker — clear or empty the field.
    Null,
    /// Text cell value.
    Text(String),
}

/// A parsed row from a file.
#[derive(Debug, Clone)]
pub struct FileRow {
    /// Column values in order (parallel to columns vec).
    pub values: Vec<CellValue>,
}

/// Result of parsing a tabular file.
#[derive(Debug, Clone)]
pub struct ParsedFile {
    /// Column headers (first row).
    pub columns: Vec<String>,
    /// Data rows.
    pub rows: Vec<FileRow>,
    /// Sheet name (Excel only, None for CSV).
    pub sheet_name: Option<String>,
    /// Available sheets (Excel only, empty for CSV).
    pub available_sheets: Vec<String>,
}

/// File format enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileFormat {
    Csv,
    Excel,
}

impl FileFormat {
    /// Detect format from file extension.
    pub fn from_path(path: &Path) -> Option<Self> {
        match path.extension()?.to_str()?.to_lowercase().as_str() {
            "csv" => Some(Self::Csv),
            "xlsx" | "xls" | "xlsm" => Some(Self::Excel),
            _ => None,
        }
    }
}
