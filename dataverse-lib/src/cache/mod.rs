//! Generic caching layer
//!
//! Provides a `CacheProvider` trait and implementations for caching
//! serialized data with TTL support. Used by the Dataverse client for
//! metadata and query result caching.

mod config;
mod memory;
mod sqlite;

pub use config::*;
pub use memory::*;
pub use sqlite::*;

use async_trait::async_trait;
use chrono::DateTime;
use chrono::Utc;

/// A cached value with metadata about when it was cached and when it expires.
#[derive(Debug, Clone)]
pub struct CachedValue {
    /// The cached data, serialized as bytes (typically via bincode).
    pub data: Vec<u8>,
    /// When this value was cached.
    pub created_at: DateTime<Utc>,
    /// When this value expires and should no longer be returned.
    pub expires_at: DateTime<Utc>,
}

impl CachedValue {
    /// Creates a new cached value.
    pub fn new(data: Vec<u8>, created_at: DateTime<Utc>, expires_at: DateTime<Utc>) -> Self {
        Self {
            data,
            created_at,
            expires_at,
        }
    }

    /// Creates a new cached value with the current time as `created_at`.
    pub fn new_now(data: Vec<u8>, expires_at: DateTime<Utc>) -> Self {
        Self {
            data,
            created_at: Utc::now(),
            expires_at,
        }
    }

    /// Creates a new cached value with a TTL from now.
    pub fn with_ttl(data: Vec<u8>, ttl: std::time::Duration) -> Self {
        let now = Utc::now();
        let expires_at = now + chrono::Duration::from_std(ttl).unwrap_or(chrono::Duration::zero());
        Self {
            data,
            created_at: now,
            expires_at,
        }
    }

    /// Returns `true` if this cached value has expired.
    pub fn is_expired(&self) -> bool {
        Utc::now() >= self.expires_at
    }
}

/// Trait for cache providers.
///
/// Implementations store and retrieve cached values by string keys.
/// The provider is responsible for:
/// - Never returning expired values from `get()`
/// - Storing values with their expiration metadata
/// - Providing garbage collection for expired entries
///
/// # Example
///
/// ```ignore
/// use dataverse_lib::cache::{CacheProvider, InMemoryCache, CachedValue};
/// use std::time::Duration;
///
/// let cache = InMemoryCache::new();
///
/// // Store a value
/// let value = CachedValue::with_ttl(b"hello".to_vec(), Duration::from_secs(60));
/// cache.set("my-key", value).await;
///
/// // Retrieve it
/// if let Some(cached) = cache.get("my-key").await {
///     println!("Got: {:?}", cached.data);
/// }
/// ```
#[async_trait]
pub trait CacheProvider: Send + Sync {
    /// Retrieves a cached value by key.
    ///
    /// Returns `None` if the key doesn't exist or the value has expired.
    /// Implementations must never return expired values.
    async fn get(&self, key: &str) -> Option<CachedValue>;

    /// Stores a value in the cache.
    async fn set(&self, key: &str, value: CachedValue);

    /// Removes a value from the cache.
    async fn remove(&self, key: &str);

    /// Clears all values from the cache.
    async fn clear(&self);

    /// Removes all expired entries from the cache.
    ///
    /// Returns the number of entries removed.
    async fn gc(&self) -> usize;
}
