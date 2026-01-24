//! Data fetching service for the record explorer.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use dataverse_lib::DataverseClient;
use dataverse_lib::api::query::odata::ODataPages;
use dataverse_lib::api::query::odata::QueryBuilder as ODataQueryBuilder;
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

/// Result of fetching records.
pub struct RecordsResult {
    pub rows: Vec<RecordRow>,
    pub pages: Option<ODataPages>,
    pub total_count: Option<usize>,
}

/// Fetch initial records page with count using the provided query builder.
pub async fn fetch_records(
    query: &ODataQueryBuilder,
    page_size: usize,
    advanced_mode: Arc<AtomicBool>,
) -> Result<RecordsResult, Error> {
    let query = query.clone().page_size(page_size);

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

    let rows = convert_records_to_rows(page.records(), advanced_mode);

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
    _id_attribute: &str,
    advanced_mode: Arc<AtomicBool>,
) -> Result<Option<(Vec<RecordRow>, bool)>, Error> {
    match pages.next().await {
        Some(Ok(page)) => {
            let rows = convert_records_to_rows(page.records(), advanced_mode);
            Ok(Some((rows, page.has_more())))
        }
        Some(Err(e)) => Err(e),
        None => Ok(None),
    }
}

/// Convert dataverse records to table rows.
fn convert_records_to_rows(
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

            for (key, _value) in record.fields() {
                let formatted = if let Some(api_formatted) = record.get_formatted(key) {
                    let raw = record
                        .get(key)
                        .map(|v| format_value(v).raw)
                        .unwrap_or_default();
                    FormattedValue::new(api_formatted, raw)
                } else {
                    record.get(key).map(|v| format_value(v)).unwrap_or_default()
                };
                row.set_cell(key.clone(), formatted);
            }

            row
        })
        .collect()
}
