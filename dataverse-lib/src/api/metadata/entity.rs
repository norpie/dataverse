//! Entity metadata builders

use std::future::Future;
use std::future::IntoFuture;
use std::pin::Pin;

use reqwest::Method;

use super::CACHE_KEY_ALL_ENTITIES;
use super::CACHE_KEY_ENTITY_CORE;
use super::CACHE_KEY_ENTITY_FULL;
use super::metadata_request;
use super::metadata_url;
use crate::DataverseClient;
use crate::cache::{self, CachedValue};
use crate::error::ApiError;
use crate::error::Error;
use crate::error::MetadataError;
use crate::model::metadata::EntityCore;
use crate::model::metadata::EntityMetadata;
use crate::model::metadata::MultiSelectPicklistAttributeMetadata;
use crate::model::metadata::PicklistAttributeMetadata;
use crate::model::metadata::StateAttributeMetadata;
use crate::model::metadata::StatusAttributeMetadata;

// =============================================================================
// EntityMetadataBuilder
// =============================================================================

/// Builder for fetching entity metadata.
pub struct EntityMetadataBuilder<'a> {
    client: &'a DataverseClient,
    entity: crate::model::Entity,
    bypass_cache: bool,
}

impl<'a> EntityMetadataBuilder<'a> {
    pub(crate) fn new(client: &'a DataverseClient, entity: crate::model::Entity) -> Self {
        Self {
            client,
            entity,
            bypass_cache: false,
        }
    }

    /// Bypass the cache and fetch directly from the API.
    pub fn bypass_cache(mut self) -> Self {
        self.bypass_cache = true;
        self
    }

    /// Execute the request.
    pub async fn execute(self) -> Result<EntityMetadata, Error> {
        let logical_name = self
            .client
            .resolve_entity_logical_name(&self.entity)
            .await?;
        let cache_key_full = format!("{}{}", CACHE_KEY_ENTITY_FULL, logical_name);

        // Check cache first (unless bypassed)
        if !self.bypass_cache
            && let Some(cache) = &self.client.inner.cache
            && let Some(cached) = cache.get(&cache_key_full).await
            && let Ok(metadata) = cache::deserialize::<EntityMetadata>(&cached.data)
        {
            return Ok(metadata);
        }

        // Fetch from API
        let metadata = fetch_entity_metadata_from_api(self.client, &logical_name).await?;

        // Cache the result
        if let Some(cache) = &self.client.inner.cache {
            let ttl = self.client.inner.cache_config.entity_metadata_ttl;

            // Cache full metadata
            if let Ok(data) = cache::serialize(&metadata) {
                cache
                    .set(&cache_key_full, CachedValue::with_ttl(data, ttl))
                    .await;
            }

            // Also cache core metadata (so CRUD resolution benefits from full fetch)
            let cache_key_core = format!("{}{}", CACHE_KEY_ENTITY_CORE, logical_name);
            if let Ok(data) = cache::serialize(&metadata.core()) {
                cache
                    .set(&cache_key_core, CachedValue::with_ttl(data, ttl))
                    .await;
            }
        }

        Ok(metadata)
    }
}

impl<'a> IntoFuture for EntityMetadataBuilder<'a> {
    type Output = Result<EntityMetadata, Error>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send + 'a>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.execute())
    }
}

// =============================================================================
// AllEntitiesBuilder
// =============================================================================

/// Builder for fetching all entity metadata.
pub struct AllEntitiesBuilder<'a> {
    client: &'a DataverseClient,
    bypass_cache: bool,
}

impl<'a> AllEntitiesBuilder<'a> {
    pub(crate) fn new(client: &'a DataverseClient) -> Self {
        Self {
            client,
            bypass_cache: false,
        }
    }

    /// Bypass the cache and fetch directly from the API.
    pub fn bypass_cache(mut self) -> Self {
        self.bypass_cache = true;
        self
    }

    /// Execute the request.
    pub async fn execute(self) -> Result<Vec<EntityMetadata>, Error> {
        // Check cache first (unless bypassed)
        if !self.bypass_cache {
            if let Some(cache) = &self.client.inner.cache {
                log::debug!("Checking cache for key '{}'", CACHE_KEY_ALL_ENTITIES);
                if let Some(cached) = cache.get(CACHE_KEY_ALL_ENTITIES).await {
                    log::debug!(
                        "Cache hit for '{}', {} bytes, attempting deserialize",
                        CACHE_KEY_ALL_ENTITIES,
                        cached.data.len()
                    );
                    match cache::deserialize::<Vec<EntityMetadata>>(&cached.data) {
                        Ok(entities) => {
                            log::debug!(
                                "Cache hit: returning {} entities from cache",
                                entities.len()
                            );
                            return Ok(entities);
                        }
                        Err(e) => {
                            log::error!("Failed to deserialize cached entities: {}", e);
                        }
                    }
                } else {
                    log::debug!("Cache miss for '{}'", CACHE_KEY_ALL_ENTITIES);
                }
            } else {
                log::warn!("No cache available for reading entities");
            }
        } else {
            log::debug!("Cache bypassed for '{}'", CACHE_KEY_ALL_ENTITIES);
        }

        // Fetch from API
        let entities = fetch_all_entities_from_api(self.client).await?;

        // Cache the result
        if let Some(cache) = &self.client.inner.cache {
            let ttl = self.client.inner.cache_config.entity_list_ttl;
            match cache::serialize(&entities) {
                Ok(data) => {
                    log::debug!(
                        "Serialized {} entities ({} bytes), caching with key '{}'",
                        entities.len(),
                        data.len(),
                        CACHE_KEY_ALL_ENTITIES
                    );
                    cache
                        .set(CACHE_KEY_ALL_ENTITIES, CachedValue::with_ttl(data, ttl))
                        .await;
                    log::debug!("Cache set completed for '{}'", CACHE_KEY_ALL_ENTITIES);
                }
                Err(e) => {
                    log::error!(
                        "Failed to serialize {} entities for caching: {}",
                        entities.len(),
                        e
                    );
                }
            }
        } else {
            log::warn!("No cache available for storing entities");
        }

        Ok(entities)
    }
}

impl<'a> IntoFuture for AllEntitiesBuilder<'a> {
    type Output = Result<Vec<EntityMetadata>, Error>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send + 'a>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.execute())
    }
}

// =============================================================================
// Internal fetch functions
// =============================================================================

/// Fetches minimal entity metadata (EntityCore) - used internally for resolution.
pub(crate) async fn fetch_entity_core(
    client: &DataverseClient,
    logical_name: &str,
    bypass_cache: bool,
) -> Result<EntityCore, Error> {
    let cache_key = format!("{}{}", CACHE_KEY_ENTITY_CORE, logical_name);

    // Check cache first (unless bypassed)
    if !bypass_cache
        && let Some(cache) = &client.inner.cache
        && let Some(cached) = cache.get(&cache_key).await
        && let Ok(core) = cache::deserialize::<EntityCore>(&cached.data)
    {
        return Ok(core);
    }

    // Fetch from API
    let core = fetch_entity_core_from_api(client, logical_name).await?;

    // Cache the result
    if let Some(cache) = &client.inner.cache {
        let ttl = client.inner.cache_config.entity_metadata_ttl;
        if let Ok(data) = cache::serialize(&core) {
            cache
                .set(&cache_key, CachedValue::with_ttl(data, ttl))
                .await;
        }
    }

    Ok(core)
}

/// Fetches minimal entity metadata directly from the API.
async fn fetch_entity_core_from_api(
    client: &DataverseClient,
    logical_name: &str,
) -> Result<EntityCore, Error> {
    let url = metadata_url(
        client,
        &format!(
            "EntityDefinitions(LogicalName='{}')?$select=LogicalName,EntitySetName,SchemaName,PrimaryIdAttribute,PrimaryNameAttribute,ObjectTypeCode",
            logical_name
        ),
    );

    let response = metadata_request(client, Method::GET, &url).await?;

    if response.status().as_u16() == 404 {
        return Err(Error::Metadata(MetadataError::EntityNotFound {
            name: logical_name.to_string(),
        }));
    }

    if !response.status().is_success() {
        let status = response.status().as_u16();
        let body = response.text().await.unwrap_or_default();
        return Err(Error::Api(ApiError::Http {
            status,
            message: body,
            code: None,
            inner: None,
        }));
    }

    let core: EntityCore = response.json().await.map_err(|e| {
        Error::Api(ApiError::Parse {
            message: format!("Failed to parse EntityCore: {}", e),
            body: None,
        })
    })?;

    Ok(core)
}

/// Fetches full entity metadata directly from the API.
///
/// This makes multiple parallel requests to fetch:
/// 1. Base entity metadata with attributes and relationships
/// 2. State attributes with their OptionSets expanded
/// 3. Status attributes with their OptionSets expanded
/// 4. Picklist attributes with their OptionSets expanded
/// 5. MultiSelect Picklist attributes with their OptionSets expanded
async fn fetch_entity_metadata_from_api(
    client: &DataverseClient,
    logical_name: &str,
) -> Result<EntityMetadata, Error> {
    // Fetch base entity metadata and typed attributes with option sets in parallel
    let (base_result, state_result, status_result, picklist_result, multi_picklist_result) = tokio::join!(
        fetch_base_entity_metadata(client, logical_name),
        fetch_state_attributes(client, logical_name),
        fetch_status_attributes(client, logical_name),
        fetch_picklist_attributes(client, logical_name),
        fetch_multi_select_picklist_attributes(client, logical_name),
    );

    // Base metadata is required
    let mut metadata = base_result?;

    // Populate typed attributes (errors are logged but don't fail the request)
    match state_result {
        Ok(attrs) => metadata.state_attributes = attrs,
        Err(e) => log::warn!(
            "Failed to fetch state attributes for {}: {}",
            logical_name,
            e
        ),
    }

    match status_result {
        Ok(attrs) => metadata.status_attributes = attrs,
        Err(e) => log::warn!(
            "Failed to fetch status attributes for {}: {}",
            logical_name,
            e
        ),
    }

    match picklist_result {
        Ok(attrs) => metadata.picklist_attributes = attrs,
        Err(e) => log::warn!(
            "Failed to fetch picklist attributes for {}: {}",
            logical_name,
            e
        ),
    }

    match multi_picklist_result {
        Ok(attrs) => metadata.multi_select_picklist_attributes = attrs,
        Err(e) => log::warn!(
            "Failed to fetch multi-select picklist attributes for {}: {}",
            logical_name,
            e
        ),
    }

    Ok(metadata)
}

/// Fetches base entity metadata with attributes and relationships.
async fn fetch_base_entity_metadata(
    client: &DataverseClient,
    logical_name: &str,
) -> Result<EntityMetadata, Error> {
    let url = metadata_url(
        client,
        &format!(
            "EntityDefinitions(LogicalName='{}')?$expand=Attributes,OneToManyRelationships,ManyToOneRelationships,ManyToManyRelationships",
            logical_name
        ),
    );

    let response = metadata_request(client, Method::GET, &url).await?;

    if response.status().as_u16() == 404 {
        return Err(Error::Metadata(MetadataError::EntityNotFound {
            name: logical_name.to_string(),
        }));
    }

    if !response.status().is_success() {
        let status = response.status().as_u16();
        let body = response.text().await.unwrap_or_default();
        return Err(Error::Api(ApiError::Http {
            status,
            message: body,
            code: None,
            inner: None,
        }));
    }

    let metadata: EntityMetadata = response.json().await.map_err(|e| {
        Error::Api(ApiError::Parse {
            message: format!("Failed to parse EntityMetadata: {}", e),
            body: None,
        })
    })?;

    Ok(metadata)
}

/// Fetches state attributes with their OptionSets expanded.
async fn fetch_state_attributes(
    client: &DataverseClient,
    entity_logical_name: &str,
) -> Result<Vec<StateAttributeMetadata>, Error> {
    let url = metadata_url(
        client,
        &format!(
            "EntityDefinitions(LogicalName='{}')/Attributes/Microsoft.Dynamics.CRM.StateAttributeMetadata?$expand=OptionSet",
            entity_logical_name
        ),
    );

    let response = metadata_request(client, Method::GET, &url).await?;

    if !response.status().is_success() {
        let status = response.status().as_u16();
        let body = response.text().await.unwrap_or_default();
        return Err(Error::Api(ApiError::Http {
            status,
            message: body,
            code: None,
            inner: None,
        }));
    }

    #[derive(serde::Deserialize)]
    struct Response {
        value: Vec<StateAttributeMetadata>,
    }

    let resp: Response = response.json().await.map_err(|e| {
        Error::Api(ApiError::Parse {
            message: format!("Failed to parse StateAttributeMetadata list: {}", e),
            body: None,
        })
    })?;

    Ok(resp.value)
}

/// Fetches status attributes with their OptionSets expanded.
async fn fetch_status_attributes(
    client: &DataverseClient,
    entity_logical_name: &str,
) -> Result<Vec<StatusAttributeMetadata>, Error> {
    let url = metadata_url(
        client,
        &format!(
            "EntityDefinitions(LogicalName='{}')/Attributes/Microsoft.Dynamics.CRM.StatusAttributeMetadata?$expand=OptionSet",
            entity_logical_name
        ),
    );

    let response = metadata_request(client, Method::GET, &url).await?;

    if !response.status().is_success() {
        let status = response.status().as_u16();
        let body = response.text().await.unwrap_or_default();
        return Err(Error::Api(ApiError::Http {
            status,
            message: body,
            code: None,
            inner: None,
        }));
    }

    #[derive(serde::Deserialize)]
    struct Response {
        value: Vec<StatusAttributeMetadata>,
    }

    let resp: Response = response.json().await.map_err(|e| {
        Error::Api(ApiError::Parse {
            message: format!("Failed to parse StatusAttributeMetadata list: {}", e),
            body: None,
        })
    })?;

    Ok(resp.value)
}

/// Fetches picklist attributes with their OptionSets expanded.
async fn fetch_picklist_attributes(
    client: &DataverseClient,
    entity_logical_name: &str,
) -> Result<Vec<PicklistAttributeMetadata>, Error> {
    let url = metadata_url(
        client,
        &format!(
            "EntityDefinitions(LogicalName='{}')/Attributes/Microsoft.Dynamics.CRM.PicklistAttributeMetadata?$expand=OptionSet",
            entity_logical_name
        ),
    );

    let response = metadata_request(client, Method::GET, &url).await?;

    if !response.status().is_success() {
        let status = response.status().as_u16();
        let body = response.text().await.unwrap_or_default();
        return Err(Error::Api(ApiError::Http {
            status,
            message: body,
            code: None,
            inner: None,
        }));
    }

    #[derive(serde::Deserialize)]
    struct Response {
        value: Vec<PicklistAttributeMetadata>,
    }

    let resp: Response = response.json().await.map_err(|e| {
        Error::Api(ApiError::Parse {
            message: format!("Failed to parse PicklistAttributeMetadata list: {}", e),
            body: None,
        })
    })?;

    Ok(resp.value)
}

/// Fetches multi-select picklist attributes with their OptionSets expanded.
async fn fetch_multi_select_picklist_attributes(
    client: &DataverseClient,
    entity_logical_name: &str,
) -> Result<Vec<MultiSelectPicklistAttributeMetadata>, Error> {
    let url = metadata_url(
        client,
        &format!(
            "EntityDefinitions(LogicalName='{}')/Attributes/Microsoft.Dynamics.CRM.MultiSelectPicklistAttributeMetadata?$expand=OptionSet",
            entity_logical_name
        ),
    );

    let response = metadata_request(client, Method::GET, &url).await?;

    if !response.status().is_success() {
        let status = response.status().as_u16();
        let body = response.text().await.unwrap_or_default();
        return Err(Error::Api(ApiError::Http {
            status,
            message: body,
            code: None,
            inner: None,
        }));
    }

    #[derive(serde::Deserialize)]
    struct Response {
        value: Vec<MultiSelectPicklistAttributeMetadata>,
    }

    let resp: Response = response.json().await.map_err(|e| {
        Error::Api(ApiError::Parse {
            message: format!(
                "Failed to parse MultiSelectPicklistAttributeMetadata list: {}",
                e
            ),
            body: None,
        })
    })?;

    Ok(resp.value)
}

/// Fetches all entity metadata from the API.
async fn fetch_all_entities_from_api(
    client: &DataverseClient,
) -> Result<Vec<EntityMetadata>, Error> {
    let url = metadata_url(client, "EntityDefinitions");

    let response = metadata_request(client, Method::GET, &url).await?;

    if !response.status().is_success() {
        let status = response.status().as_u16();
        let body = response.text().await.unwrap_or_default();
        return Err(Error::Api(ApiError::Http {
            status,
            message: body,
            code: None,
            inner: None,
        }));
    }

    // The response is wrapped in a "value" array
    #[derive(serde::Deserialize)]
    struct Response {
        value: Vec<EntityMetadata>,
    }

    let resp: Response = response.json().await.map_err(|e| {
        Error::Api(ApiError::Parse {
            message: format!("Failed to parse EntityMetadata list: {}", e),
            body: None,
        })
    })?;

    Ok(resp.value)
}
