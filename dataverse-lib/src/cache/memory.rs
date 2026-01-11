//! In-memory cache implementation using DashMap

use async_trait::async_trait;
use dashmap::DashMap;

use super::CacheProvider;
use super::CachedValue;

/// An in-memory cache backed by a concurrent hash map.
///
/// This is the default cache implementation. It's fast and thread-safe,
/// but data is lost when the process exits.
///
/// # Example
///
/// ```
/// use dataverse_lib::cache::InMemoryCache;
///
/// let cache = InMemoryCache::new();
/// ```
#[derive(Debug, Default)]
pub struct InMemoryCache {
    store: DashMap<String, CachedValue>,
}

impl InMemoryCache {
    /// Creates a new empty in-memory cache.
    pub fn new() -> Self {
        Self {
            store: DashMap::new(),
        }
    }

    /// Creates a new in-memory cache with the specified initial capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            store: DashMap::with_capacity(capacity),
        }
    }

    /// Returns the number of entries in the cache (including expired ones).
    pub fn len(&self) -> usize {
        self.store.len()
    }

    /// Returns `true` if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.store.is_empty()
    }
}

#[async_trait]
impl CacheProvider for InMemoryCache {
    async fn get(&self, key: &str) -> Option<CachedValue> {
        let entry = self.store.get(key)?;
        let value = entry.value();

        if value.is_expired() {
            drop(entry);
            self.store.remove(key);
            None
        } else {
            Some(value.clone())
        }
    }

    async fn set(&self, key: &str, value: CachedValue) {
        self.store.insert(key.to_string(), value);
    }

    async fn remove(&self, key: &str) {
        self.store.remove(key);
    }

    async fn clear(&self) {
        self.store.clear();
    }

    async fn gc(&self) -> usize {
        let mut removed = 0;
        self.store.retain(|_, value| {
            if value.is_expired() {
                removed += 1;
                false
            } else {
                true
            }
        });
        removed
    }
}
