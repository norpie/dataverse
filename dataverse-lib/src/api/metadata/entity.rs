//! Entity metadata builders

use std::future::Future;
use std::future::IntoFuture;
use std::pin::Pin;

use reqwest::Method;

use super::metadata_request;
use super::metadata_url;
use super::CACHE_KEY_ALL_ENTITIES;
use super::CACHE_KEY_ENTITY_CORE;
use super::CACHE_KEY_ENTITY_FULL;
use crate::cache::CachedValue;
use crate::error::ApiError;
use crate::error::Error;
use crate::error::MetadataError;
use crate::model::metadata::EntityCore;
use crate::model::metadata::EntityMetadata;
use crate::DataverseClient;

// =============================================================================
// EntityMetadataBuilder
// =============================================================================

/// Builder for fetching entity metadata.
pub struct EntityMetadataBuilder<'a> {
    client: &'a DataverseClient,
    logical_name: String,
    bypass_cache: bool,
}

impl<'a> EntityMetadataBuilder<'a> {
    pub(crate) fn new(client: &'a DataverseClient, logical_name: String) -> Self {
        Self {
            client,
            logical_name,
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
        let cache_key_full = format!("{}{}", CACHE_KEY_ENTITY_FULL, self.logical_name);

        // Check cache first (unless bypassed)
        if !self.bypass_cache {
            if let Some(cache) = &self.client.inner.cache {
                if let Some(cached) = cache.get(&cache_key_full).await {
                    if let Ok(metadata) = bincode::deserialize::<EntityMetadata>(&cached.data) {
                        return Ok(metadata);
                    }
                }
            }
        }

        // Fetch from API
        let metadata = fetch_entity_metadata_from_api(self.client, &self.logical_name).await?;

        // Cache the result
        if let Some(cache) = &self.client.inner.cache {
            let ttl = self.client.inner.cache_config.metadata_ttl;

            // Cache full metadata
            if let Ok(data) = bincode::serialize(&metadata) {
                cache
                    .set(&cache_key_full, CachedValue::with_ttl(data, ttl))
                    .await;
            }

            // Also cache core metadata (so CRUD resolution benefits from full fetch)
            let cache_key_core = format!("{}{}", CACHE_KEY_ENTITY_CORE, self.logical_name);
            if let Ok(data) = bincode::serialize(&metadata.core) {
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
                if let Some(cached) = cache.get(CACHE_KEY_ALL_ENTITIES).await {
                    if let Ok(entities) = bincode::deserialize::<Vec<EntityMetadata>>(&cached.data)
                    {
                        return Ok(entities);
                    }
                }
            }
        }

        // Fetch from API
        let entities = fetch_all_entities_from_api(self.client).await?;

        // Cache the result
        if let Some(cache) = &self.client.inner.cache {
            let ttl = self.client.inner.cache_config.metadata_ttl;
            if let Ok(data) = bincode::serialize(&entities) {
                cache
                    .set(CACHE_KEY_ALL_ENTITIES, CachedValue::with_ttl(data, ttl))
                    .await;
            }
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
    if !bypass_cache {
        if let Some(cache) = &client.inner.cache {
            if let Some(cached) = cache.get(&cache_key).await {
                if let Ok(core) = bincode::deserialize::<EntityCore>(&cached.data) {
                    return Ok(core);
                }
            }
        }
    }

    // Fetch from API
    let core = fetch_entity_core_from_api(client, logical_name).await?;

    // Cache the result
    if let Some(cache) = &client.inner.cache {
        if let Ok(data) = bincode::serialize(&core) {
            let ttl = client.inner.cache_config.metadata_ttl;
            cache.set(&cache_key, CachedValue::with_ttl(data, ttl)).await;
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
async fn fetch_entity_metadata_from_api(
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

/// Fetches all entity metadata from the API.
async fn fetch_all_entities_from_api(client: &DataverseClient) -> Result<Vec<EntityMetadata>, Error> {
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
