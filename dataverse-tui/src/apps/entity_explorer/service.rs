//! Data fetching service for the entity explorer.

use dataverse_lib::DataverseClient;
use dataverse_lib::error::Error;

/// Result of fetching all entities.
pub struct AllEntitiesResult {
    /// Sorted list of (logical_name, display_name) tuples.
    pub entities: Vec<(String, String)>,
}

/// Fetch all entities from Dataverse.
pub async fn fetch_all_entities(client: &DataverseClient) -> Result<AllEntitiesResult, Error> {
    let all_entities = client.metadata().all_entities().await?;

    let mut entities: Vec<(String, String)> = all_entities
        .iter()
        .map(|e| {
            let display = e.display_name.text().unwrap_or(&e.logical_name).to_string();
            (e.logical_name.clone(), display)
        })
        .collect();

    // Sort by display name
    entities.sort_by(|a, b| a.1.to_lowercase().cmp(&b.1.to_lowercase()));

    Ok(AllEntitiesResult { entities })
}
