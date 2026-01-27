//! CSV read/write operations.

use std::path::Path;

use dataverse_lib::model::Record;

use crate::formatting::format_value;

use super::{FileIoError, FileRow, ParsedFile};

/// Write records to CSV file.
///
/// - Header row: column names
/// - Data rows: `.raw` representation for all values
/// - Null values written as empty strings
///
/// This is a blocking operation - caller should use spawn_blocking.
pub fn write_csv(path: &Path, records: &[Record], columns: &[String]) -> Result<(), FileIoError> {
    let mut writer = csv::Writer::from_path(path)?;

    // Header row
    writer.write_record(columns)?;

    // Data rows
    for record in records {
        let row: Vec<String> = columns
            .iter()
            .map(|col| {
                record
                    .get(col)
                    .map(|v| format_value(v).raw)
                    .unwrap_or_default()
            })
            .collect();
        writer.write_record(&row)?;
    }

    writer.flush()?;
    Ok(())
}

/// Parse CSV file into columns and string rows.
///
/// - First row treated as headers
/// - Empty cells become None
///
/// This is a blocking operation - caller should use spawn_blocking.
pub fn read_csv(path: &Path) -> Result<ParsedFile, FileIoError> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_path(path)?;

    let headers = reader.headers()?.clone();
    let columns: Vec<String> = headers.iter().map(|s| s.to_string()).collect();

    if columns.is_empty() {
        return Err(FileIoError::EmptyFile);
    }

    let mut rows = Vec::new();
    let column_count = columns.len();
    for result in reader.records() {
        let record = result?;
        let mut values: Vec<Option<String>> = record
            .iter()
            .map(|s| {
                let trimmed = s.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            })
            .collect();

        // Pad to match column count
        values.resize(column_count, None);

        rows.push(FileRow { values });
    }

    Ok(ParsedFile {
        columns,
        rows,
        sheet_name: None,
        available_sheets: Vec::new(),
    })
}
