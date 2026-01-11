//! Concurrency limiting for simultaneous requests.

use std::sync::Arc;

use tokio::sync::Semaphore;
use tokio::sync::SemaphorePermit;

/// Limits the number of concurrent requests.
///
/// Wraps a `tokio::sync::Semaphore` to enforce a maximum number of
/// simultaneous requests. Default limit is 52 (Dataverse's concurrent
/// request limit per user per web server).
///
/// # Example
///
/// ```
/// use dataverse_lib::rate_limit::ConcurrencyLimiter;
///
/// let limiter = ConcurrencyLimiter::new(10);
/// assert_eq!(limiter.limit(), 10);
/// assert_eq!(limiter.available(), 10);
/// ```
#[derive(Clone)]
pub struct ConcurrencyLimiter {
    semaphore: Arc<Semaphore>,
    limit: usize,
}

impl ConcurrencyLimiter {
    /// Creates a new concurrency limiter with the specified limit.
    pub fn new(limit: usize) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(limit)),
            limit,
        }
    }

    /// Acquires a permit, waiting if necessary.
    ///
    /// The permit is released when dropped.
    pub async fn acquire(&self) -> SemaphorePermit<'_> {
        self.semaphore
            .acquire()
            .await
            .expect("semaphore should not be closed")
    }

    /// Returns the configured limit.
    pub fn limit(&self) -> usize {
        self.limit
    }

    /// Returns the number of available permits.
    pub fn available(&self) -> usize {
        self.semaphore.available_permits()
    }
}

impl Default for ConcurrencyLimiter {
    fn default() -> Self {
        Self::new(52)
    }
}
