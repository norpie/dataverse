//! Entity/attribute metadata operations
//!
//! This module provides methods for fetching entity metadata from the Dataverse API.
//! Metadata is cached to avoid repeated API calls.

use reqwest::header::HeaderMap;
use reqwest::header::HeaderValue;
use reqwest::Method;

use crate::cache::CachedValue;
use crate::error::ApiError;
use crate::error::Error;
use crate::error::MetadataError;
use crate::model::metadata::EntityCore;
use crate::model::metadata::EntityMetadata;
use crate::DataverseClient;

/// Cache key prefix for minimal entity metadata (EntityCore).
const CACHE_KEY_ENTITY_CORE: &str = "entity_core:";

/// Cache key prefix for full entity metadata (EntityMetadata).
const CACHE_KEY_ENTITY_FULL: &str = "entity_full:";

impl DataverseClient {
    /// Resolves an entity logical name to its entity set name.
    ///
    /// This is used internally by CRUD operations to build API URLs.
    /// Uses cached metadata if available, otherwise fetches minimal metadata.
    ///
    /// # Errors
    ///
    /// Returns an error if the entity doesn't exist or the API call fails.
    pub async fn resolve_entity_set_name(&self, logical_name: &str) -> Result<String, Error> {
        let core = self.fetch_entity_core(logical_name).await?;
        Ok(core.entity_set_name)
    }

    /// Fetches minimal entity metadata (EntityCore).
    ///
    /// Checks the cache first. On cache miss, fetches from the API and caches the result.
    /// This is a fast fetch that only retrieves the fields needed for CRUD operations.
    pub async fn fetch_entity_core(&self, logical_name: &str) -> Result<EntityCore, Error> {
        let cache_key = format!("{}{}", CACHE_KEY_ENTITY_CORE, logical_name);

        // Check cache first
        if let Some(cache) = &self.inner.cache {
            if let Some(cached) = cache.get(&cache_key).await {
                if let Ok(core) = bincode::deserialize::<EntityCore>(&cached.data) {
                    return Ok(core);
                }
            }
        }

        // Fetch from API
        let core = self.fetch_entity_core_from_api(logical_name).await?;

        // Cache the result
        if let Some(cache) = &self.inner.cache {
            if let Ok(data) = bincode::serialize(&core) {
                let ttl = self.inner.cache_config.metadata_ttl;
                cache.set(&cache_key, CachedValue::with_ttl(data, ttl)).await;
            }
        }

        Ok(core)
    }

    /// Fetches full entity metadata including attributes and relationships.
    ///
    /// Checks the cache first. On cache miss, fetches from the API and caches
    /// both the full metadata and the core metadata (for CRUD resolution).
    pub async fn fetch_entity_metadata(&self, logical_name: &str) -> Result<EntityMetadata, Error> {
        let cache_key_full = format!("{}{}", CACHE_KEY_ENTITY_FULL, logical_name);

        // Check cache first
        if let Some(cache) = &self.inner.cache {
            if let Some(cached) = cache.get(&cache_key_full).await {
                if let Ok(metadata) = bincode::deserialize::<EntityMetadata>(&cached.data) {
                    return Ok(metadata);
                }
            }
        }

        // Fetch from API
        let metadata = self.fetch_entity_metadata_from_api(logical_name).await?;

        // Cache both full and core
        if let Some(cache) = &self.inner.cache {
            let ttl = self.inner.cache_config.metadata_ttl;

            // Cache full metadata
            if let Ok(data) = bincode::serialize(&metadata) {
                cache
                    .set(&cache_key_full, CachedValue::with_ttl(data, ttl))
                    .await;
            }

            // Also cache core metadata (so CRUD resolution benefits from full fetch)
            let cache_key_core = format!("{}{}", CACHE_KEY_ENTITY_CORE, logical_name);
            if let Ok(data) = bincode::serialize(&metadata.core) {
                cache
                    .set(&cache_key_core, CachedValue::with_ttl(data, ttl))
                    .await;
            }
        }

        Ok(metadata)
    }

    /// Fetches minimal entity metadata directly from the API (no cache).
    async fn fetch_entity_core_from_api(&self, logical_name: &str) -> Result<EntityCore, Error> {
        let url = format!(
            "{}/api/data/{}/EntityDefinitions(LogicalName='{}')?$select=LogicalName,EntitySetName,SchemaName,PrimaryIdAttribute,PrimaryNameAttribute,ObjectTypeCode",
            self.inner.base_url.trim_end_matches('/'),
            self.inner.api_version,
            logical_name
        );

        let response = self.metadata_request(Method::GET, &url).await?;

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

    /// Fetches full entity metadata directly from the API (no cache).
    async fn fetch_entity_metadata_from_api(
        &self,
        logical_name: &str,
    ) -> Result<EntityMetadata, Error> {
        let url = format!(
            "{}/api/data/{}/EntityDefinitions(LogicalName='{}')?$expand=Attributes,OneToManyRelationships,ManyToOneRelationships,ManyToManyRelationships",
            self.inner.base_url.trim_end_matches('/'),
            self.inner.api_version,
            logical_name
        );

        let response = self.metadata_request(Method::GET, &url).await?;

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

    /// Makes an authenticated request to the metadata API.
    async fn metadata_request(
        &self,
        method: Method,
        url: &str,
    ) -> Result<reqwest::Response, Error> {
        let token = self
            .inner
            .token_provider
            .get_token(&self.inner.base_url)
            .await?;

        let mut headers = HeaderMap::new();
        headers.insert("OData-MaxVersion", HeaderValue::from_static("4.0"));
        headers.insert("OData-Version", HeaderValue::from_static("4.0"));
        headers.insert("Accept", HeaderValue::from_static("application/json"));

        let mut request = self
            .inner
            .http_client
            .request(method, url)
            .headers(headers)
            .bearer_auth(&token.access_token);

        if let Some(timeout) = self.inner.timeout {
            request = request.timeout(timeout);
        }

        let response = request.send().await.map_err(ApiError::from)?;
        Ok(response)
    }

    /// Invalidates cached metadata for an entity.
    ///
    /// Call this if you know the entity metadata has changed (e.g., after
    /// adding/removing attributes via the schema API).
    pub async fn invalidate_entity_metadata(&self, logical_name: &str) {
        if let Some(cache) = &self.inner.cache {
            let cache_key_core = format!("{}{}", CACHE_KEY_ENTITY_CORE, logical_name);
            let cache_key_full = format!("{}{}", CACHE_KEY_ENTITY_FULL, logical_name);
            cache.remove(&cache_key_core).await;
            cache.remove(&cache_key_full).await;
        }
    }
}
