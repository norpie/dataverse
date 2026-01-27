//! Excel read/write operations.

use std::path::Path;

use calamine::{open_workbook, Data, Reader, Xlsx};
use rust_decimal::prelude::ToPrimitive;
use rust_xlsxwriter::{Format, Workbook};

use dataverse_lib::model::{Record, Value};

use crate::formatting::format_value;

use super::{FileIoError, FileRow, ParsedFile};

/// Maximum length for Excel sheet names.
const MAX_SHEET_NAME_LENGTH: usize = 31;

/// Characters forbidden in Excel sheet names.
const FORBIDDEN_SHEET_CHARS: &[char] = &['\\', '/', '*', '?', ':', '[', ']'];

/// Validate an Excel sheet name.
fn validate_sheet_name(name: &str) -> Result<(), FileIoError> {
    if name.is_empty() {
        return Err(FileIoError::InvalidSheetName {
            name: name.to_string(),
            reason: "sheet name cannot be empty".to_string(),
        });
    }

    if name.len() > MAX_SHEET_NAME_LENGTH {
        return Err(FileIoError::InvalidSheetName {
            name: name.to_string(),
            reason: format!(
                "sheet name exceeds {} characters (length: {})",
                MAX_SHEET_NAME_LENGTH,
                name.len()
            ),
        });
    }

    for c in FORBIDDEN_SHEET_CHARS {
        if name.contains(*c) {
            return Err(FileIoError::InvalidSheetName {
                name: name.to_string(),
                reason: format!("sheet name contains forbidden character '{}'", c),
            });
        }
    }

    Ok(())
}

/// Write records to Excel file.
///
/// - Sheet name: provided entity name (must be <= 31 chars, no `\ / * ? : [ ]`)
/// - Row 0: Header row (bold)
/// - Row 1+: Data rows with native types where possible
/// - Column widths: fixed at 20
///
/// This is a blocking operation - caller should use spawn_blocking.
pub fn write_excel(
    path: &Path,
    records: &[Record],
    columns: &[String],
    sheet_name: &str,
) -> Result<(), FileIoError> {
    validate_sheet_name(sheet_name)?;

    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet().set_name(sheet_name)?;

    // Formats
    let header_format = Format::new().set_bold();
    let date_format = Format::new().set_num_format("yyyy-mm-dd hh:mm");

    // Column widths (default 20)
    for col_idx in 0..columns.len() {
        worksheet.set_column_width(col_idx as u16, 20)?;
    }

    // Header row
    for (col_idx, col_name) in columns.iter().enumerate() {
        worksheet.write_string_with_format(0, col_idx as u16, col_name, &header_format)?;
    }

    // Data rows
    for (row_idx, record) in records.iter().enumerate() {
        let excel_row = (row_idx + 1) as u32; // +1 for header

        for (col_idx, col_name) in columns.iter().enumerate() {
            let excel_col = col_idx as u16;

            let Some(value) = record.get(col_name) else {
                continue; // Leave cell empty
            };

            write_cell(worksheet, excel_row, excel_col, value, &date_format)?;
        }
    }

    workbook.save(path)?;
    Ok(())
}

/// Write a single cell value with appropriate Excel type.
fn write_cell(
    worksheet: &mut rust_xlsxwriter::Worksheet,
    row: u32,
    col: u16,
    value: &Value,
    date_format: &Format,
) -> Result<(), FileIoError> {
    match value {
        Value::Null => {
            // Leave empty
        }
        Value::Bool(b) => {
            worksheet.write_boolean(row, col, *b)?;
        }
        Value::Int(n) => {
            worksheet.write_number(row, col, *n as f64)?;
        }
        Value::Long(n) => {
            worksheet.write_number(row, col, *n as f64)?;
        }
        Value::Float(n) => {
            worksheet.write_number(row, col, *n)?;
        }
        Value::Decimal(d) => {
            if let Some(n) = d.to_f64() {
                worksheet.write_number(row, col, n)?;
            } else {
                worksheet.write_string(row, col, &d.to_string())?;
            }
        }
        Value::Money(m) => {
            if let Some(n) = m.value().to_f64() {
                worksheet.write_number(row, col, n)?;
            } else {
                worksheet.write_string(row, col, &m.value().to_string())?;
            }
        }
        Value::DateTime(dt) => {
            let excel_dt = rust_xlsxwriter::ExcelDateTime::from_timestamp(dt.timestamp())?;
            worksheet.write_datetime_with_format(row, col, &excel_dt, date_format)?;
        }
        // All other types: use raw string representation
        _ => {
            let formatted = format_value(value).raw;
            if !formatted.is_empty() {
                worksheet.write_string(row, col, &formatted)?;
            }
        }
    }
    Ok(())
}

/// List available sheet names in an Excel file.
///
/// This is a blocking operation - caller should use spawn_blocking.
pub fn list_sheets(path: &Path) -> Result<Vec<String>, FileIoError> {
    let workbook: Xlsx<_> = open_workbook(path).map_err(calamine_xlsx_to_error)?;
    Ok(workbook.sheet_names().to_vec())
}

/// Parse Excel file into columns and string rows.
///
/// - If sheet_name is None, uses the first sheet
/// - First row treated as headers
/// - Empty cells become None
///
/// This is a blocking operation - caller should use spawn_blocking.
pub fn read_excel(path: &Path, sheet_name: Option<&str>) -> Result<ParsedFile, FileIoError> {
    let mut workbook: Xlsx<_> = open_workbook(path).map_err(calamine_xlsx_to_error)?;
    let available_sheets = workbook.sheet_names().to_vec();

    if available_sheets.is_empty() {
        return Err(FileIoError::NoSheets);
    }

    let target_sheet = match sheet_name {
        Some(name) => {
            if !available_sheets.contains(&name.to_string()) {
                return Err(FileIoError::SheetNotFound(name.to_string()));
            }
            name.to_string()
        }
        None => available_sheets[0].clone(),
    };

    let range = workbook
        .worksheet_range(&target_sheet)
        .map_err(calamine_xlsx_to_error)?;

    let mut row_iter = range.rows();

    // First row = headers
    let header_row = row_iter.next().ok_or(FileIoError::EmptyFile)?;
    let columns: Vec<String> = header_row.iter().map(cell_to_string).collect();

    if columns.is_empty() || columns.iter().all(|s| s.is_empty()) {
        return Err(FileIoError::EmptyFile);
    }

    // Data rows
    let mut rows = Vec::new();
    for row in row_iter {
        let values: Vec<Option<String>> = row
            .iter()
            .map(|cell| {
                let s = cell_to_string(cell);
                if s.is_empty() {
                    None
                } else {
                    Some(s)
                }
            })
            .collect();

        // Pad or truncate to match column count
        let mut values = values;
        values.resize(columns.len(), None);

        rows.push(FileRow { values });
    }

    Ok(ParsedFile {
        columns,
        rows,
        sheet_name: Some(target_sheet),
        available_sheets,
    })
}

/// Convert calamine XlsxError to FileIoError.
fn calamine_xlsx_to_error(e: calamine::XlsxError) -> FileIoError {
    FileIoError::ExcelRead(calamine::Error::Xlsx(e))
}

/// Convert calamine Data cell to string.
fn cell_to_string(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::String(s) => s.trim().to_string(),
        Data::Int(n) => n.to_string(),
        Data::Float(n) => {
            // Avoid unnecessary decimals for whole numbers
            if n.fract() == 0.0 {
                (*n as i64).to_string()
            } else {
                n.to_string()
            }
        }
        Data::Bool(b) => b.to_string(),
        Data::DateTime(dt) => {
            // calamine returns ExcelDateTime, convert to ISO
            if let Some(naive) = dt.as_datetime() {
                naive.format("%Y-%m-%dT%H:%M:%S").to_string()
            } else {
                format!("{}", dt.as_f64())
            }
        }
        Data::Error(e) => format!("#ERROR:{:?}", e),
        Data::DateTimeIso(s) => s.to_string(),
        Data::DurationIso(s) => s.to_string(),
    }
}
