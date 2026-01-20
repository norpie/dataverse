//! Data fetching service for the record explorer.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use dataverse_lib::DataverseClient;
use dataverse_lib::api::query::odata::ODataPages;
use dataverse_lib::error::Error;
use dataverse_lib::model::Entity;
use dataverse_lib::model::metadata::{AttributeMetadata, EntityMetadata};

use crate::formatting::{FormattedValue, format_value};

use super::row::{EntityData, RecordRow};

/// Fetch entity metadata and build EntityData.
pub async fn fetch_entity_data(
    client: &DataverseClient,
    logical_name: &str,
) -> Result<EntityData, Error> {
    let entity = client.metadata().entity(logical_name).await?;

    // Filter readable attributes for field selection
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

/// Build sorted field options from entity data.
pub fn build_field_options(entity_data: &EntityData, advanced: bool) -> Vec<(String, String)> {
    let mut options: Vec<(String, String)> = entity_data
        .readable_fields
        .iter()
        .map(|a| {
            let label = if advanced {
                a.logical_name.clone()
            } else {
                a.display_name.text().unwrap_or(&a.logical_name).to_string()
            };
            (a.logical_name.clone(), label)
        })
        .collect();

    options.sort_by(|a, b| a.1.to_lowercase().cmp(&b.1.to_lowercase()));
    options
}

/// Get default columns for an entity.
pub fn default_columns(entity: &EntityMetadata) -> Vec<String> {
    let mut cols = Vec::new();
    if let Some(primary) = &entity.core.primary_name_attribute {
        cols.push(primary.clone());
    }
    cols.push("createdon".to_string());
    cols.push("modifiedon".to_string());
    cols
}

/// Result of fetching records.
pub struct RecordsResult {
    pub rows: Vec<RecordRow>,
    pub pages: Option<ODataPages>,
    pub total_count: Option<usize>,
}

/// Fetch initial records page with count.
pub async fn fetch_records(
    client: &DataverseClient,
    entity: &EntityMetadata,
    columns: &[String],
    page_size: usize,
    advanced_mode: Arc<AtomicBool>,
) -> Result<RecordsResult, Error> {
    // Build select list including ID
    let mut select_cols: Vec<&str> = columns.iter().map(|s| s.as_str()).collect();
    select_cols.push(&entity.core.primary_id_attribute);

    // Create query
    let query = client
        .query(Entity::Set(entity.core.entity_set_name.clone()))
        .select(&select_cols)
        .page_size(page_size);

    // Run count query and first page fetch in parallel
    let count_query = query.clone();
    let (count_result, mut pages) =
        tokio::join!(count_query.count(), async { query.into_async_iter() });

    let total_count = count_result.ok();

    let page = match pages.next().await {
        Some(Ok(p)) => p,
        Some(Err(e)) => return Err(e),
        None => {
            return Ok(RecordsResult {
                rows: Vec::new(),
                pages: None,
                total_count,
            });
        }
    };

    let rows = convert_records_to_rows(
        page.records(),
        &entity.core.primary_id_attribute,
        advanced_mode,
    );

    let pages = if page.has_more() { Some(pages) } else { None };

    Ok(RecordsResult {
        rows,
        pages,
        total_count,
    })
}

/// Fetch the next page of records.
pub async fn fetch_next_page(
    pages: &mut ODataPages,
    id_attribute: &str,
    advanced_mode: Arc<AtomicBool>,
) -> Result<Option<(Vec<RecordRow>, bool)>, Error> {
    match pages.next().await {
        Some(Ok(page)) => {
            let rows = convert_records_to_rows(page.records(), id_attribute, advanced_mode);
            Ok(Some((rows, page.has_more())))
        }
        Some(Err(e)) => Err(e),
        None => Ok(None),
    }
}

/// Convert dataverse records to table rows.
pub fn convert_records_to_rows(
    records: &[dataverse_lib::model::Record],
    id_attribute: &str,
    advanced_mode: Arc<AtomicBool>,
) -> Vec<RecordRow> {
    records
        .iter()
        .enumerate()
        .map(|(idx, record)| {
            let id = record
                .id()
                .map(|u| u.to_string())
                .or_else(|| {
                    record
                        .get_guid(id_attribute)
                        .ok()
                        .flatten()
                        .map(|u| u.to_string())
                })
                .unwrap_or_else(|| format!("unknown-{}", idx));

            let mut row = RecordRow::new(id, advanced_mode.clone());

            // Populate cells - prefer formatted values from API, fall back to our formatting
            for (key, _value) in record.fields() {
                let formatted = if let Some(api_formatted) = record.get_formatted(key) {
                    // API provided a formatted value - use it for display, get raw from Value
                    let raw = record
                        .get(key)
                        .map(|v| format_value(v).raw)
                        .unwrap_or_default();
                    FormattedValue::new(api_formatted, raw)
                } else {
                    // No API formatted value - use our formatting
                    record.get(key).map(|v| format_value(v)).unwrap_or_default()
                };
                row.set_cell(key.clone(), formatted);
            }

            row
        })
        .collect()
}
