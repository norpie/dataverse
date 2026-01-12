//! Global option set metadata builders

use std::future::Future;
use std::future::IntoFuture;
use std::pin::Pin;

use reqwest::Method;

use super::metadata_request;
use super::metadata_url;
use super::CACHE_KEY_ALL_GLOBAL_OPTIONSETS;
use super::CACHE_KEY_GLOBAL_OPTIONSET;
use crate::cache::CachedValue;
use crate::error::ApiError;
use crate::error::Error;
use crate::error::MetadataError;
use crate::model::metadata::GlobalOptionSetMetadata;
use crate::DataverseClient;

// =============================================================================
// GlobalOptionSetBuilder
// =============================================================================

/// Builder for fetching a single global option set's metadata.
pub struct GlobalOptionSetBuilder<'a> {
    client: &'a DataverseClient,
    name: String,
    bypass_cache: bool,
}

impl<'a> GlobalOptionSetBuilder<'a> {
    pub(crate) fn new(client: &'a DataverseClient, name: String) -> Self {
        Self {
            client,
            name,
            bypass_cache: false,
        }
    }

    /// Bypass the cache and fetch directly from the API.
    pub fn bypass_cache(mut self) -> Self {
        self.bypass_cache = true;
        self
    }

    /// Execute the request.
    pub async fn execute(self) -> Result<GlobalOptionSetMetadata, Error> {
        let cache_key = format!("{}{}", CACHE_KEY_GLOBAL_OPTIONSET, self.name);

        // Check cache first (unless bypassed)
        if !self.bypass_cache {
            if let Some(cache) = &self.client.inner.cache {
                if let Some(cached) = cache.get(&cache_key).await {
                    if let Ok(os) = bincode::deserialize::<GlobalOptionSetMetadata>(&cached.data) {
                        return Ok(os);
                    }
                }
            }
        }

        // Fetch from API
        let os = fetch_global_option_set_from_api(self.client, &self.name).await?;

        // Cache the result
        if let Some(cache) = &self.client.inner.cache {
            let ttl = self.client.inner.cache_config.metadata_ttl;
            if let Ok(data) = bincode::serialize(&os) {
                cache
                    .set(&cache_key, CachedValue::with_ttl(data, ttl))
                    .await;
            }
        }

        Ok(os)
    }
}

impl<'a> IntoFuture for GlobalOptionSetBuilder<'a> {
    type Output = Result<GlobalOptionSetMetadata, Error>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send + 'a>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.execute())
    }
}

// =============================================================================
// AllGlobalOptionSetsBuilder
// =============================================================================

/// Builder for fetching all global option sets.
pub struct AllGlobalOptionSetsBuilder<'a> {
    client: &'a DataverseClient,
    bypass_cache: bool,
}

impl<'a> AllGlobalOptionSetsBuilder<'a> {
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
    pub async fn execute(self) -> Result<Vec<GlobalOptionSetMetadata>, Error> {
        // Check cache first (unless bypassed)
        if !self.bypass_cache {
            if let Some(cache) = &self.client.inner.cache {
                if let Some(cached) = cache.get(CACHE_KEY_ALL_GLOBAL_OPTIONSETS).await {
                    if let Ok(option_sets) =
                        bincode::deserialize::<Vec<GlobalOptionSetMetadata>>(&cached.data)
                    {
                        return Ok(option_sets);
                    }
                }
            }
        }

        // Fetch from API
        let option_sets = fetch_all_global_option_sets_from_api(self.client).await?;

        // Cache the result
        if let Some(cache) = &self.client.inner.cache {
            let ttl = self.client.inner.cache_config.metadata_ttl;
            if let Ok(data) = bincode::serialize(&option_sets) {
                cache
                    .set(
                        CACHE_KEY_ALL_GLOBAL_OPTIONSETS,
                        CachedValue::with_ttl(data, ttl),
                    )
                    .await;
            }
        }

        Ok(option_sets)
    }
}

impl<'a> IntoFuture for AllGlobalOptionSetsBuilder<'a> {
    type Output = Result<Vec<GlobalOptionSetMetadata>, Error>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send + 'a>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.execute())
    }
}

// =============================================================================
// Internal fetch functions
// =============================================================================

/// Fetches a single global option set from the API.
async fn fetch_global_option_set_from_api(
    client: &DataverseClient,
    name: &str,
) -> Result<GlobalOptionSetMetadata, Error> {
    let url = metadata_url(
        client,
        &format!("GlobalOptionSetDefinitions(Name='{}')", name),
    );

    let response = metadata_request(client, Method::GET, &url).await?;

    if response.status().as_u16() == 404 {
        return Err(Error::Metadata(MetadataError::OptionSetNotFound {
            name: name.to_string(),
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

    let os: GlobalOptionSetMetadata = response.json().await.map_err(|e| {
        Error::Api(ApiError::Parse {
            message: format!("Failed to parse GlobalOptionSetMetadata: {}", e),
            body: None,
        })
    })?;

    Ok(os)
}

/// Fetches all global option sets from the API.
async fn fetch_all_global_option_sets_from_api(
    client: &DataverseClient,
) -> Result<Vec<GlobalOptionSetMetadata>, Error> {
    let url = metadata_url(client, "GlobalOptionSetDefinitions");

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
        value: Vec<GlobalOptionSetMetadata>,
    }

    let resp: Response = response.json().await.map_err(|e| {
        Error::Api(ApiError::Parse {
            message: format!("Failed to parse GlobalOptionSetMetadata list: {}", e),
            body: None,
        })
    })?;

    Ok(resp.value)
}
