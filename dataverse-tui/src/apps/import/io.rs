//! Import I/O utilities for parsing files and converting to operations.

use std::collections::HashMap;

use dataverse_lib::api::{Op, Operation};
use dataverse_lib::model::metadata::{AttributeMetadata, AttributeType};
use dataverse_lib::model::types::{EntityBinding, MultiSelectOptionSetValue, OptionSetValue};
use dataverse_lib::model::{Entity, Record, Value};
use uuid::Uuid;

use crate::file_io::{CellValue, ConvertError, FileRow, string_to_value};

/// Error during import parsing/conversion.
#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("Invalid odata.bind value: {0}")]
    InvalidODataBind(String),

    #[error("Row {row}: {error}")]
    RowError { row: usize, error: String },

    #[error("Primary key not found in attributes")]
    PrimaryKeyNotFound,

    #[error("Conversion error: {0}")]
    Convert(#[from] ConvertError),
}

/// Column information after parsing headers.
#[derive(Debug, Clone)]
pub struct ColumnInfo {
    /// Original header text.
    pub header: String,
    /// Field name (stripped of @odata.bind suffix).
    pub field_name: String,
    /// Whether this is a lookup column.
    pub is_lookup: bool,
}

/// Parse headers to identify lookup columns.
pub fn parse_headers(headers: &[String]) -> Vec<ColumnInfo> {
    headers
        .iter()
        .map(|header| {
            let header_trimmed = header.trim().to_string();
            if let Some(field_name) = header_trimmed.strip_suffix("@odata.bind") {
                ColumnInfo {
                    header: header_trimmed.clone(),
                    field_name: field_name.to_string(),
                    is_lookup: true,
                }
            } else {
                ColumnInfo {
                    header: header_trimmed.clone(),
                    field_name: header_trimmed,
                    is_lookup: false,
                }
            }
        })
        .collect()
}

/// Parse an odata.bind value like "/accounts(abc-123-def-456)".
///
/// Returns (entity_set_name, uuid).
pub fn parse_odata_bind(value: &str) -> Result<(String, Uuid), ImportError> {
    let value = value.trim();

    // Must start with /
    if !value.starts_with('/') {
        return Err(ImportError::InvalidODataBind(value.to_string()));
    }

    // Find opening paren
    let Some(paren_start) = value.find('(') else {
        return Err(ImportError::InvalidODataBind(value.to_string()));
    };

    // Find closing paren
    let Some(paren_end) = value.find(')') else {
        return Err(ImportError::InvalidODataBind(value.to_string()));
    };

    if paren_end <= paren_start + 1 {
        return Err(ImportError::InvalidODataBind(value.to_string()));
    }

    // Extract entity set name (between / and ())
    let entity_set = value[1..paren_start].to_string();

    // Extract GUID
    let guid_str = &value[paren_start + 1..paren_end];
    let guid =
        Uuid::parse_str(guid_str).map_err(|_| ImportError::InvalidODataBind(value.to_string()))?;

    Ok((entity_set, guid))
}

/// Convert a single row to a Record.
///
/// Returns (Option<Uuid>, Record):
/// - None = row has no primary key (will be a Create)
/// - Some(id) = row has primary key (will be an Upsert)
pub fn row_to_record(
    row_num: usize,
    row: &FileRow,
    columns: &[ColumnInfo],
    attributes: &HashMap<String, AttributeMetadata>,
    entity_name: &str,
    primary_key_field: &str,
) -> Result<(Option<Uuid>, Record), ImportError> {
    let mut record = Record::new(entity_name);
    let mut primary_key_id: Option<Uuid> = None;

    for (col_idx, col_info) in columns.iter().enumerate() {
        let cell_value = row.values.get(col_idx).unwrap_or(&CellValue::Empty);

        // Check if this is the primary key column
        if col_info.field_name == primary_key_field {
            if let CellValue::Text(cell_str) = cell_value {
                // Parse as UUID
                if let Ok(id) = Uuid::parse_str(cell_str) {
                    primary_key_id = Some(id);
                }
            }
            // Don't add primary key to record fields
            continue;
        }

        let Some(attr) = attributes.get(&col_info.field_name) else {
            return Err(row_error(
                row_num,
                format!("Unknown attribute: {}", col_info.field_name),
            ));
        };

        match cell_value {
            CellValue::Empty => {
                // Unchanged.
            }
            CellValue::Null => {
                record = record.set(&col_info.field_name, null_value_for_attribute(attr));
            }
            CellValue::Text(cell_str) => {
                let value = convert_text_value(row_num, cell_str, attr, &col_info.field_name)?;
                record = record.set(&col_info.field_name, value);
            }
        }
    }

    Ok((primary_key_id, record))
}

/// Convert rows to Operations (Create or Upsert based on presence of ID).
///
/// Returns (operations, errors).
pub fn rows_to_operations(
    rows: &[FileRow],
    columns: &[ColumnInfo],
    attributes: &HashMap<String, AttributeMetadata>,
    entity: Entity,
    primary_key_field: &str,
) -> (Vec<Operation>, Vec<ImportError>) {
    let mut operations = Vec::new();
    let mut errors = Vec::new();

    for (row_idx, row) in rows.iter().enumerate() {
        match row_to_record(
            row_idx + 1,
            row,
            columns,
            attributes,
            entity.name(),
            primary_key_field,
        ) {
            Ok((id_opt, record)) => {
                let op = if let Some(id) = id_opt {
                    // Has ID → Upsert
                    Op::upsert(entity.clone(), id, record).build()
                } else {
                    // No ID → Create
                    Op::create(entity.clone(), record).build()
                };
                operations.push(op);
            }
            Err(e) => {
                errors.push(e);
            }
        }
    }

    (operations, errors)
}

/// Count operations by type (create vs upsert).
///
/// Returns (create_count, upsert_count).
pub fn count_operation_types(
    rows: &[FileRow],
    columns: &[ColumnInfo],
    primary_key_field: &str,
) -> (usize, usize) {
    let mut create_count = 0;
    let mut upsert_count = 0;

    // Find primary key column index
    let pk_col_idx = columns
        .iter()
        .position(|col| col.field_name == primary_key_field);

    let Some(pk_idx) = pk_col_idx else {
        // No primary key column - all creates
        return (rows.len(), 0);
    };

    for row in rows {
        let has_id = matches!(
            row.values.get(pk_idx),
            Some(CellValue::Text(s)) if Uuid::parse_str(s).is_ok()
        );

        if has_id {
            upsert_count += 1;
        } else {
            create_count += 1;
        }
    }

    (create_count, upsert_count)
}

/// Find the primary key field name from attributes.
pub fn find_primary_key(attributes: &[AttributeMetadata]) -> Result<String, ImportError> {
    attributes
        .iter()
        .find(|attr| attr.is_primary_id)
        .map(|attr| attr.logical_name.clone())
        .ok_or(ImportError::PrimaryKeyNotFound)
}

fn row_error(row_num: usize, error: impl Into<String>) -> ImportError {
    ImportError::RowError {
        row: row_num,
        error: error.into(),
    }
}

fn null_value_for_attribute(attr: &AttributeMetadata) -> Value {
    match attr.attribute_type {
        AttributeType::String | AttributeType::Memo => Value::String(String::new()),
        _ => Value::Null,
    }
}

fn convert_text_value(
    row_num: usize,
    cell_str: &str,
    attr: &AttributeMetadata,
    field_name: &str,
) -> Result<Value, ImportError> {
    match attr.attribute_type {
        AttributeType::String | AttributeType::Memo => Ok(Value::String(cell_str.to_string())),

        AttributeType::Lookup | AttributeType::Customer | AttributeType::Owner => {
            let (entity_set, id) = parse_odata_bind(cell_str)
                .map_err(|e| row_error(row_num, format!("{}: {}", field_name, e)))?;
            Ok(Value::EntityBinding(EntityBinding::new(&entity_set, id)))
        }

        AttributeType::Picklist | AttributeType::State | AttributeType::Status => {
            let value = string_to_value(cell_str, &attr.attribute_type, None)
                .map_err(|e| row_error(row_num, format!("{}: {}", field_name, e)))?;
            validate_option_value(row_num, field_name, attr, &value)?;
            Ok(value)
        }

        AttributeType::MultiSelectPicklist => {
            let value = string_to_value(cell_str, &attr.attribute_type, None)
                .map_err(|e| row_error(row_num, format!("{}: {}", field_name, e)))?;
            validate_option_value(row_num, field_name, attr, &value)?;
            Ok(value)
        }

        _ => string_to_value(cell_str, &attr.attribute_type, None)
            .map_err(|e| row_error(row_num, format!("{}: {}", field_name, e))),
    }
}

fn validate_option_value(
    row_num: usize,
    field_name: &str,
    attr: &AttributeMetadata,
    value: &Value,
) -> Result<(), ImportError> {
    let Some(option_set) = attr.options() else {
        return Ok(());
    };

    match value {
        Value::OptionSet(OptionSetValue { value, .. }) => {
            if option_set.options.iter().any(|opt| opt.value == *value) {
                Ok(())
            } else {
                Err(row_error(
                    row_num,
                    format!("{}: invalid option set value {}", field_name, value),
                ))
            }
        }
        Value::MultiOptionSet(MultiSelectOptionSetValue { values, .. }) => {
            let invalid: Vec<i32> = values
                .iter()
                .copied()
                .filter(|value| !option_set.options.iter().any(|opt| opt.value == *value))
                .collect();

            if invalid.is_empty() {
                Ok(())
            } else {
                Err(row_error(
                    row_num,
                    format!("{}: invalid option set values {:?}", field_name, invalid),
                ))
            }
        }
        _ => Ok(()),
    }
}
