//! Relationship metadata builders

use std::future::Future;
use std::future::IntoFuture;
use std::pin::Pin;

use reqwest::Method;

use super::metadata_request;
use super::metadata_url;
use super::CACHE_KEY_RELATIONSHIP;
use crate::cache::CachedValue;
use crate::error::ApiError;
use crate::error::Error;
use crate::error::MetadataError;
use crate::model::metadata::ManyToManyRelationship;
use crate::model::metadata::OneToManyRelationship;
use crate::model::metadata::RelationshipMetadata;
use crate::DataverseClient;

// =============================================================================
// RelationshipMetadataBuilder
// =============================================================================

/// Builder for fetching a single relationship's metadata.
pub struct RelationshipMetadataBuilder<'a> {
    client: &'a DataverseClient,
    schema_name: String,
    bypass_cache: bool,
}

impl<'a> RelationshipMetadataBuilder<'a> {
    pub(crate) fn new(client: &'a DataverseClient, schema_name: String) -> Self {
        Self {
            client,
            schema_name,
            bypass_cache: false,
        }
    }

    /// Bypass the cache and fetch directly from the API.
    pub fn bypass_cache(mut self) -> Self {
        self.bypass_cache = true;
        self
    }

    /// Execute the request.
    pub async fn execute(self) -> Result<RelationshipMetadata, Error> {
        let cache_key = format!("{}{}", CACHE_KEY_RELATIONSHIP, self.schema_name);

        // Check cache first (unless bypassed)
        if !self.bypass_cache {
            if let Some(cache) = &self.client.inner.cache {
                if let Some(cached) = cache.get(&cache_key).await {
                    if let Ok(rel) = bincode::deserialize::<RelationshipMetadata>(&cached.data) {
                        return Ok(rel);
                    }
                }
            }
        }

        // Fetch from API
        let rel = fetch_relationship_from_api(self.client, &self.schema_name).await?;

        // Cache the result
        if let Some(cache) = &self.client.inner.cache {
            let ttl = self.client.inner.cache_config.metadata_ttl;
            if let Ok(data) = bincode::serialize(&rel) {
                cache
                    .set(&cache_key, CachedValue::with_ttl(data, ttl))
                    .await;
            }
        }

        Ok(rel)
    }
}

impl<'a> IntoFuture for RelationshipMetadataBuilder<'a> {
    type Output = Result<RelationshipMetadata, Error>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send + 'a>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.execute())
    }
}

// =============================================================================
// RelationshipsBuilder
// =============================================================================

/// Builder for fetching all relationships of an entity.
pub struct RelationshipsBuilder<'a> {
    client: &'a DataverseClient,
    entity: String,
    bypass_cache: bool,
}

impl<'a> RelationshipsBuilder<'a> {
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
    pub async fn execute(self) -> Result<Vec<RelationshipMetadata>, Error> {
        // Fetch entity metadata which includes all relationships
        let mut builder = super::entity::EntityMetadataBuilder::new(self.client, self.entity);
        if self.bypass_cache {
            builder = builder.bypass_cache();
        }
        let entity_meta = builder.execute().await?;

        // Combine all relationships into a single vector
        let mut relationships = Vec::new();

        for rel in entity_meta.one_to_many_relationships {
            relationships.push(RelationshipMetadata::OneToMany(rel));
        }

        for rel in entity_meta.many_to_one_relationships {
            relationships.push(RelationshipMetadata::OneToMany(rel));
        }

        for rel in entity_meta.many_to_many_relationships {
            relationships.push(RelationshipMetadata::ManyToMany(rel));
        }

        Ok(relationships)
    }
}

impl<'a> IntoFuture for RelationshipsBuilder<'a> {
    type Output = Result<Vec<RelationshipMetadata>, Error>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send + 'a>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.execute())
    }
}

// =============================================================================
// Internal fetch functions
// =============================================================================

/// Fetches a single relationship's metadata from the API.
///
/// Tries OneToManyRelationshipDefinitions first, then ManyToManyRelationshipDefinitions.
async fn fetch_relationship_from_api(
    client: &DataverseClient,
    schema_name: &str,
) -> Result<RelationshipMetadata, Error> {
    // Try one-to-many first
    let url = metadata_url(
        client,
        &format!(
            "RelationshipDefinitions(SchemaName='{}')",
            schema_name
        ),
    );

    let response = metadata_request(client, Method::GET, &url).await?;

    if response.status().as_u16() == 404 {
        return Err(Error::Metadata(MetadataError::RelationshipNotFound {
            name: schema_name.to_string(),
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

    // The API returns a generic relationship definition.
    // We need to determine the type from the response.
    let body = response.text().await.map_err(|e| {
        Error::Api(ApiError::Parse {
            message: format!("Failed to read response body: {}", e),
            body: None,
        })
    })?;

    // Try to parse as OneToMany first
    if let Ok(rel) = serde_json::from_str::<OneToManyRelationship>(&body) {
        return Ok(RelationshipMetadata::OneToMany(rel));
    }

    // Try ManyToMany
    if let Ok(rel) = serde_json::from_str::<ManyToManyRelationship>(&body) {
        return Ok(RelationshipMetadata::ManyToMany(rel));
    }

    // If neither worked, return a parse error
    Err(Error::Api(ApiError::Parse {
        message: "Failed to parse relationship metadata".to_string(),
        body: Some(body),
    }))
}
