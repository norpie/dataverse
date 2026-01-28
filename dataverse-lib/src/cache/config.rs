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
///     .with_entity_list_ttl(Duration::from_secs(7200))
///     .with_query_ttl(Duration::from_secs(60));
/// ```
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// TTL for entity list (all_entities bulk fetch).
    ///
    /// Default: 24 hours
    pub entity_list_ttl: Duration,

    /// TTL for single entity metadata.
    ///
    /// Default: 6 hours
    pub entity_metadata_ttl: Duration,

    /// TTL for single attribute metadata.
    ///
    /// Default: 6 hours
    pub attribute_metadata_ttl: Duration,

    /// TTL for global option sets.
    ///
    /// Default: 12 hours
    pub global_optionset_ttl: Duration,

    /// TTL for relationships.
    ///
    /// Default: 12 hours
    pub relationship_ttl: Duration,

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
            entity_list_ttl: Duration::from_secs(86400), // 24 hours
            entity_metadata_ttl: Duration::from_secs(21600), // 6 hours
            attribute_metadata_ttl: Duration::from_secs(21600), // 6 hours
            global_optionset_ttl: Duration::from_secs(43200), // 12 hours
            relationship_ttl: Duration::from_secs(43200), // 12 hours
            query_ttl: Duration::from_secs(300),         // 5 minutes
            record_ttl: Duration::from_secs(300),        // 5 minutes
        }
    }
}

impl CacheConfig {
    /// Creates a new cache config with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the entity list TTL.
    pub fn with_entity_list_ttl(mut self, ttl: Duration) -> Self {
        self.entity_list_ttl = ttl;
        self
    }

    /// Sets the entity metadata TTL.
    pub fn with_entity_metadata_ttl(mut self, ttl: Duration) -> Self {
        self.entity_metadata_ttl = ttl;
        self
    }

    /// Sets the attribute metadata TTL.
    pub fn with_attribute_metadata_ttl(mut self, ttl: Duration) -> Self {
        self.attribute_metadata_ttl = ttl;
        self
    }

    /// Sets the global option set TTL.
    pub fn with_global_optionset_ttl(mut self, ttl: Duration) -> Self {
        self.global_optionset_ttl = ttl;
        self
    }

    /// Sets the relationship TTL.
    pub fn with_relationship_ttl(mut self, ttl: Duration) -> Self {
        self.relationship_ttl = ttl;
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
            entity_list_ttl: Duration::ZERO,
            entity_metadata_ttl: Duration::ZERO,
            attribute_metadata_ttl: Duration::ZERO,
            global_optionset_ttl: Duration::ZERO,
            relationship_ttl: Duration::ZERO,
            query_ttl: Duration::ZERO,
            record_ttl: Duration::ZERO,
        }
    }
}
