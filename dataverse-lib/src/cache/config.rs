//! Cache configuration

use std::time::Duration;

/// Configuration for cache TTL (time-to-live) settings.
///
/// Controls how long different types of data are cached before expiring.
///
/// # Example
///
/// ```
/// use std::time::Duration;
/// use dataverse_lib::cache::CacheConfig;
///
/// let config = CacheConfig::default()
///     .with_metadata_ttl(Duration::from_secs(7200))
///     .with_query_ttl(Duration::from_secs(60));
/// ```
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// TTL for entity metadata (logical name → set name, etc.).
    ///
    /// Default: 1 hour
    pub metadata_ttl: Duration,

    /// TTL for query results (OData, FetchXML).
    ///
    /// Default: 5 minutes
    pub query_ttl: Duration,

    /// TTL for individual record retrievals.
    ///
    /// Default: 5 minutes
    pub record_ttl: Duration,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            metadata_ttl: Duration::from_secs(3600),  // 1 hour
            query_ttl: Duration::from_secs(300),      // 5 minutes
            record_ttl: Duration::from_secs(300),     // 5 minutes
        }
    }
}

impl CacheConfig {
    /// Creates a new cache config with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the metadata TTL.
    pub fn with_metadata_ttl(mut self, ttl: Duration) -> Self {
        self.metadata_ttl = ttl;
        self
    }

    /// Sets the query TTL.
    pub fn with_query_ttl(mut self, ttl: Duration) -> Self {
        self.query_ttl = ttl;
        self
    }

    /// Sets the record TTL.
    pub fn with_record_ttl(mut self, ttl: Duration) -> Self {
        self.record_ttl = ttl;
        self
    }

    /// Creates a config with no caching (zero TTLs).
    pub fn no_cache() -> Self {
        Self {
            metadata_ttl: Duration::ZERO,
            query_ttl: Duration::ZERO,
            record_ttl: Duration::ZERO,
        }
    }
}
