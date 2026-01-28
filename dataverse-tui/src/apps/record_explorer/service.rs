//! Data fetching service for the record explorer.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use dataverse_lib::DataverseClient;
use dataverse_lib::error::Error;
use dataverse_lib::model::metadata::{AttributeMetadata, EntityMetadata};

use crate::formatting::{FormattedValue, format_value};

use super::row::{EntityData, RecordRow};

/// Fetch entity metadata and build EntityData.
pub async fn fetch_entity_data(
    client: &DataverseClient,
    logical_name: &str,
) -> Result<EntityData, Error> {
    let entity = client.metadata().entity(logical_name).await?;

    let readable_fields: Vec<AttributeMetadata> = entity
        .attributes
        .iter()
        .filter(|a| a.is_valid_for_read && a.attribute_of.is_none())
        .cloned()
        .collect();

    Ok(EntityData {
        metadata: entity,
        readable_fields,
    })
}

/// Get default columns for an entity (when no select is specified).
pub fn default_columns(entity: &EntityMetadata) -> Vec<String> {
    let mut cols = Vec::new();
    if let Some(primary) = &entity.primary_name_attribute {
        cols.push(primary.clone());
    }
    cols.push("createdon".to_string());
    cols.push("modifiedon".to_string());
    cols
}

/// Convert dataverse records to table rows.
pub fn convert_records_to_rows(
    records: &[dataverse_lib::model::Record],
    advanced_mode: Arc<AtomicBool>,
) -> Vec<RecordRow> {
    records
        .iter()
        .enumerate()
        .map(|(idx, record)| {
            let id = record
                .id()
                .map(|u| u.to_string())
                .unwrap_or_else(|| format!("row-{}", idx));

            let mut row = RecordRow::new(id, advanced_mode.clone());

            for key in record.fields().keys() {
                let formatted = if let Some(api_formatted) = record.get_formatted(key) {
                    let raw = record
                        .get(key)
                        .map(|v| format_value(v).raw)
                        .unwrap_or_default();
                    FormattedValue::new(api_formatted, raw)
                } else {
                    record.get(key).map(format_value).unwrap_or_default()
                };
                row.set_cell(key.clone(), formatted);
            }

            row
        })
        .collect()
}
