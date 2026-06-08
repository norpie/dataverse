//! CSV read/write operations.

use std::path::Path;

use super::{CellValue, FileIoError, FileRow, ParsedFile};

/// Write rows to CSV file.
///
/// - First row: headers
/// - Remaining rows: data
///
/// This is a blocking operation - caller should use spawn_blocking.
pub fn write_csv(path: &Path, headers: &[String], rows: &[Vec<String>]) -> Result<(), FileIoError> {
    let mut writer = csv::Writer::from_path(path)?;

    writer.write_record(headers)?;

    for row in rows {
        writer.write_record(row)?;
    }

    writer.flush()?;
    Ok(())
}

/// Parse CSV file into columns and string rows.
///
/// - First row treated as headers
/// - Blank cells become `CellValue::Empty`
/// - Literal `null` becomes `CellValue::Null`
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
        let mut values: Vec<CellValue> = record
            .iter()
            .map(|s| {
                let trimmed = s.trim();
                if trimmed.is_empty() {
                    CellValue::Empty
                } else if trimmed == "null" {
                    CellValue::Null
                } else {
                    CellValue::Text(trimmed.to_string())
                }
            })
            .collect();

        // Pad to match column count
        values.resize(column_count, CellValue::Empty);

        rows.push(FileRow { values });
    }

    Ok(ParsedFile {
        columns,
        rows,
        sheet_name: None,
        available_sheets: Vec::new(),
    })
}
