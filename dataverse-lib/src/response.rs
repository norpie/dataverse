//! Response wrapper with cache status

use chrono::DateTime;
use chrono::Utc;

/// A response from the Dataverse client that includes cache status information.
///
/// All fetch operations (queries, retrieves, metadata lookups) return this wrapper
/// so callers can determine whether the data came from cache or was freshly fetched.
///
/// # Example
///
/// ```ignore
/// let response = client.retrieve(Entity::logical("account"), id).await?;
///
/// if response.is_cached() {
///     println!("Data from cache, cached at {:?}", response.cached_at());
/// }
///
/// let record = response.into_inner();
/// ```
#[derive(Debug, Clone)]
pub struct Response<T> {
    data: T,
    /// Information about whether this response came from cache.
    pub cache: CacheStatus,
}

impl<T> Response<T> {
    /// Creates a new response with no cache involvement.
    pub fn new(data: T) -> Self {
        Self {
            data,
            cache: CacheStatus::None,
        }
    }

    /// Creates a new response indicating a cache miss (fresh fetch, now cached).
    pub fn cache_miss(data: T, cached_at: DateTime<Utc>, expires_at: DateTime<Utc>) -> Self {
        Self {
            data,
            cache: CacheStatus::Miss {
                cached_at,
                expires_at,
            },
        }
    }

    /// Creates a new response indicating a cache hit.
    pub fn cache_hit(data: T, cached_at: DateTime<Utc>, expires_at: DateTime<Utc>) -> Self {
        Self {
            data,
            cache: CacheStatus::Hit {
                cached_at,
                expires_at,
            },
        }
    }

    /// Returns `true` if this response came from the cache.
    pub fn is_cached(&self) -> bool {
        matches!(self.cache, CacheStatus::Hit { .. })
    }

    /// Returns `true` if this was a fresh fetch (cache miss or cache disabled).
    pub fn is_fresh(&self) -> bool {
        !self.is_cached()
    }

    /// Returns when the data was cached, if applicable.
    pub fn cached_at(&self) -> Option<DateTime<Utc>> {
        match &self.cache {
            CacheStatus::None => None,
            CacheStatus::Miss { cached_at, .. } | CacheStatus::Hit { cached_at, .. } => {
                Some(*cached_at)
            }
        }
    }

    /// Returns when the cached data expires, if applicable.
    pub fn expires_at(&self) -> Option<DateTime<Utc>> {
        match &self.cache {
            CacheStatus::None => None,
            CacheStatus::Miss { expires_at, .. } | CacheStatus::Hit { expires_at, .. } => {
                Some(*expires_at)
            }
        }
    }

    /// Returns a reference to the inner data.
    pub fn data(&self) -> &T {
        &self.data
    }

    /// Consumes the response and returns the inner data.
    pub fn into_inner(self) -> T {
        self.data
    }

    /// Maps the inner data using the provided function.
    pub fn map<U, F: FnOnce(T) -> U>(self, f: F) -> Response<U> {
        Response {
            data: f(self.data),
            cache: self.cache,
        }
    }
}

/// Cache status for a response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheStatus {
    /// Cache was disabled or bypassed for this request.
    None,
    /// Cache miss - data was freshly fetched and is now cached.
    Miss {
        /// When the data was cached.
        cached_at: DateTime<Utc>,
        /// When the cached data will expire.
        expires_at: DateTime<Utc>,
    },
    /// Cache hit - data was returned from cache.
    Hit {
        /// When the data was originally cached.
        cached_at: DateTime<Utc>,
        /// When the cached data will expire.
        expires_at: DateTime<Utc>,
    },
}

impl CacheStatus {
    /// Returns `true` if this is a cache hit.
    pub fn is_hit(&self) -> bool {
        matches!(self, Self::Hit { .. })
    }

    /// Returns `true` if this is a cache miss.
    pub fn is_miss(&self) -> bool {
        matches!(self, Self::Miss { .. })
    }

    /// Returns `true` if caching was not involved.
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}
