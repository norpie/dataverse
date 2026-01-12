//! Attribute metadata builders

use std::future::Future;
use std::future::IntoFuture;
use std::pin::Pin;

use reqwest::Method;

use super::metadata_request;
use super::metadata_url;
use super::CACHE_KEY_ATTRIBUTE;
use crate::cache::CachedValue;
use crate::error::ApiError;
use crate::error::Error;
use crate::error::MetadataError;
use crate::model::metadata::AttributeMetadata;
use crate::DataverseClient;

// =============================================================================
// AttributeMetadataBuilder
// =============================================================================

/// Builder for fetching a single attribute's metadata.
pub struct AttributeMetadataBuilder<'a> {
    client: &'a DataverseClient,
    entity: String,
    attribute: String,
    bypass_cache: bool,
}

impl<'a> AttributeMetadataBuilder<'a> {
    pub(crate) fn new(client: &'a DataverseClient, entity: String, attribute: String) -> Self {
        Self {
            client,
            entity,
            attribute,
            bypass_cache: false,
        }
    }

    /// Bypass the cache and fetch directly from the API.
    pub fn bypass_cache(mut self) -> Self {
        self.bypass_cache = true;
        self
    }

    /// Execute the request.
    pub async fn execute(self) -> Result<AttributeMetadata, Error> {
        let cache_key = format!("{}{}:{}", CACHE_KEY_ATTRIBUTE, self.entity, self.attribute);

        // Check cache first (unless bypassed)
        if !self.bypass_cache {
            if let Some(cache) = &self.client.inner.cache {
                if let Some(cached) = cache.get(&cache_key).await {
                    if let Ok(attr) = bincode::deserialize::<AttributeMetadata>(&cached.data) {
                        return Ok(attr);
                    }
                }
            }
        }

        // Fetch from API
        let attr =
            fetch_attribute_metadata_from_api(self.client, &self.entity, &self.attribute).await?;

        // Cache the result
        if let Some(cache) = &self.client.inner.cache {
            let ttl = self.client.inner.cache_config.metadata_ttl;
            if let Ok(data) = bincode::serialize(&attr) {
                cache
                    .set(&cache_key, CachedValue::with_ttl(data, ttl))
                    .await;
            }
        }

        Ok(attr)
    }
}

impl<'a> IntoFuture for AttributeMetadataBuilder<'a> {
    type Output = Result<AttributeMetadata, Error>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send + 'a>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.execute())
    }
}

// =============================================================================
// AttributesBuilder
// =============================================================================

/// Builder for fetching all attributes of an entity.
pub struct AttributesBuilder<'a> {
    client: &'a DataverseClient,
    entity: String,
    bypass_cache: bool,
}

impl<'a> AttributesBuilder<'a> {
    pub(crate) fn new(client: &'a DataverseClient, entity: String) -> Self {
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
    pub async fn execute(self) -> Result<Vec<AttributeMetadata>, Error> {
        // For attributes, we fetch via the entity metadata endpoint
        // since that's more efficient than fetching each attribute individually.
        // We could add a dedicated cache key for "all attributes of entity X"
        // but for now we just fetch from the entity metadata.
        let attrs = fetch_attributes_from_api(self.client, &self.entity).await?;
        Ok(attrs)
    }
}

impl<'a> IntoFuture for AttributesBuilder<'a> {
    type Output = Result<Vec<AttributeMetadata>, Error>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send + 'a>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.execute())
    }
}

// =============================================================================
// Internal fetch functions
// =============================================================================

/// Fetches a single attribute's metadata from the API.
async fn fetch_attribute_metadata_from_api(
    client: &DataverseClient,
    entity: &str,
    attribute: &str,
) -> Result<AttributeMetadata, Error> {
    let url = metadata_url(
        client,
        &format!(
            "EntityDefinitions(LogicalName='{}')/Attributes(LogicalName='{}')",
            entity, attribute
        ),
    );

    let response = metadata_request(client, Method::GET, &url).await?;

    if response.status().as_u16() == 404 {
        return Err(Error::Metadata(MetadataError::AttributeNotFound {
            entity: entity.to_string(),
            attribute: attribute.to_string(),
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

    let attr: AttributeMetadata = response.json().await.map_err(|e| {
        Error::Api(ApiError::Parse {
            message: format!("Failed to parse AttributeMetadata: {}", e),
            body: None,
        })
    })?;

    Ok(attr)
}

/// Fetches all attributes for an entity from the API.
async fn fetch_attributes_from_api(
    client: &DataverseClient,
    entity: &str,
) -> Result<Vec<AttributeMetadata>, Error> {
    let url = metadata_url(
        client,
        &format!("EntityDefinitions(LogicalName='{}')/Attributes", entity),
    );

    let response = metadata_request(client, Method::GET, &url).await?;

    if response.status().as_u16() == 404 {
        return Err(Error::Metadata(MetadataError::EntityNotFound {
            name: entity.to_string(),
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

    // The response is wrapped in a "value" array
    #[derive(serde::Deserialize)]
    struct Response {
        value: Vec<AttributeMetadata>,
    }

    let resp: Response = response.json().await.map_err(|e| {
        Error::Api(ApiError::Parse {
            message: format!("Failed to parse AttributeMetadata list: {}", e),
            body: None,
        })
    })?;

    Ok(resp.value)
}
