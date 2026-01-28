//! Import I/O utilities for parsing files and converting to operations.

use std::collections::HashMap;

use dataverse_lib::api::{Op, Operation};
use dataverse_lib::model::metadata::AttributeMetadata;
use dataverse_lib::model::types::EntityBinding;
use dataverse_lib::model::{Entity, Record, Value};
use uuid::Uuid;

use crate::file_io::{string_to_value, ConvertError, FileRow};

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

    // Extract entity set name (between / and ()
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
    row: &FileRow,
    columns: &[ColumnInfo],
    attributes: &HashMap<String, AttributeMetadata>,
    entity_name: &str,
    primary_key_field: &str,
) -> Result<(Option<Uuid>, Record), ImportError> {
    let mut record = Record::new(entity_name);
    let mut primary_key_id: Option<Uuid> = None;

    for (col_idx, col_info) in columns.iter().enumerate() {
        // Get cell value
        let cell_value = row.values.get(col_idx).and_then(|v| v.as_ref());
        let Some(cell_str) = cell_value else {
            // Empty cell - skip
            continue;
        };

        // Check if this is the primary key column
        if col_info.field_name == primary_key_field {
            // Parse as UUID
            if let Ok(id) = Uuid::parse_str(cell_str) {
                primary_key_id = Some(id);
            }
            // Don't add primary key to record fields
            continue;
        }

        // Convert value based on whether it's a lookup
        let value = if col_info.is_lookup {
            // Parse odata.bind format
            let (entity_set, id) = parse_odata_bind(cell_str)?;
            Value::EntityBinding(EntityBinding::new(&entity_set, id))
        } else {
            // Get attribute metadata for type info
            let attr = attributes.get(&col_info.field_name).ok_or_else(|| {
                ImportError::RowError {
                    row: 0, // row number added by caller
                    error: format!("Unknown attribute: {}", col_info.field_name),
                }
            })?;

            // Convert using string_to_value
            let target_set = if attr.is_lookup() {
                attr.targets.first().map(|s| s.as_str())
            } else {
                None
            };

            string_to_value(cell_str, &attr.attribute_type, target_set)?
        };

        // Add to record (skip nulls)
        if !matches!(value, Value::Null) {
            record = record.set(&col_info.field_name, value);
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
        match row_to_record(row, columns, attributes, entity.name(), primary_key_field) {
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
                // Wrap error with row number
                let row_error = ImportError::RowError {
                    row: row_idx + 1, // 1-based for user display
                    error: e.to_string(),
                };
                errors.push(row_error);
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
        let has_id = row
            .values
            .get(pk_idx)
            .and_then(|v| v.as_ref())
            .and_then(|s| Uuid::parse_str(s).ok())
            .is_some();

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
