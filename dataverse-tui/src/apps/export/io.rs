//! Export I/O utilities for transforming records to export format.

use std::collections::HashMap;

use dataverse_lib::model::{Record, Value};

use crate::formatting::format_value;

/// Lookup column info: maps column name to target entity set name.
/// For polymorphic lookups (Customer, Owner), this is the first target.
pub type LookupColumns = HashMap<String, String>;

/// Format a value for export, using odata.bind format for lookups.
fn format_export_value(value: &Value, column: &str, lookup_columns: &LookupColumns) -> String {
    // Check if this column is a lookup
    if let Some(entity_set) = lookup_columns.get(column) {
        // Extract GUID from the value
        let guid = match value {
            Value::EntityReference(r) => Some(r.id),
            Value::Guid(g) => Some(*g),
            Value::Null => None,
            _ => None,
        };

        if let Some(id) = guid {
            return format!("/{}({})", entity_set, id);
        }
    }

    // Not a lookup or no value - use default formatting
    format_value(value).raw
}

/// Transform records to string rows for export.
pub fn records_to_rows(
    records: &[Record],
    columns: &[String],
    lookup_columns: &LookupColumns,
) -> Vec<Vec<String>> {
    records
        .iter()
        .map(|record| {
            columns
                .iter()
                .map(|col| {
                    record
                        .get(col)
                        .map(|v| format_export_value(v, col, lookup_columns))
                        .unwrap_or_default()
                })
                .collect()
        })
        .collect()
}

/// Transform column headers: add @odata.bind suffix for lookup columns.
pub fn transform_headers(columns: &[String], lookup_columns: &LookupColumns) -> Vec<String> {
    columns
        .iter()
        .map(|col| {
            if lookup_columns.contains_key(col) {
                format!("{}@odata.bind", col)
            } else {
                col.clone()
            }
        })
        .collect()
}
