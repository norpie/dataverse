//! Sliding window rate limiter.

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;
use tokio::time::Instant;

/// Sliding window rate limiter.
///
/// Tracks request timestamps and enforces a maximum number of requests
/// within a sliding time window. Default is 6000 requests per 5 minutes
/// (Dataverse's service protection limit).
///
/// This limiter is cheap to clone and can be shared across multiple clients
/// when they use the same user credentials and should share the rate limit quota.
///
/// # Example
///
/// ```
/// use std::time::Duration;
/// use dataverse_lib::rate_limit::RateLimiter;
///
/// // Default: 6000 requests per 5 minutes
/// let limiter = RateLimiter::default();
///
/// // Custom: 100 requests per minute
/// let custom = RateLimiter::new(100, Duration::from_secs(60));
/// ```
#[derive(Clone)]
pub struct RateLimiter {
    inner: Arc<RateLimiterInner>,
}

struct RateLimiterInner {
    state: Mutex<RateLimiterState>,
    capacity: u32,
    window: Duration,
}

struct RateLimiterState {
    /// Timestamps of recent requests within the window.
    timestamps: VecDeque<Instant>,
}

impl RateLimiter {
    /// Creates a new rate limiter.
    ///
    /// # Arguments
    ///
    /// * `capacity` - Maximum requests allowed within the window
    /// * `window` - Duration of the sliding window
    pub fn new(capacity: u32, window: Duration) -> Self {
        Self {
            inner: Arc::new(RateLimiterInner {
                state: Mutex::new(RateLimiterState {
                    timestamps: VecDeque::with_capacity(capacity as usize),
                }),
                capacity,
                window,
            }),
        }
    }

    /// Acquires permission to make a request.
    ///
    /// If the rate limit is exceeded, waits until a slot becomes available.
    pub async fn acquire(&self) {
        loop {
            let wait_time = {
                let mut state = self.inner.state.lock().await;
                let now = Instant::now();

                // Remove expired timestamps
                let cutoff = now - self.inner.window;
                while let Some(&ts) = state.timestamps.front() {
                    if ts < cutoff {
                        state.timestamps.pop_front();
                    } else {
                        break;
                    }
                }

                // Check if we have capacity
                if (state.timestamps.len() as u32) < self.inner.capacity {
                    state.timestamps.push_back(now);
                    return;
                }

                // Calculate wait time until oldest request expires
                if let Some(&oldest) = state.timestamps.front() {
                    let expires_at = oldest + self.inner.window;
                    if expires_at > now {
                        Some(expires_at - now)
                    } else {
                        None
                    }
                } else {
                    None
                }
            };

            // Wait outside the lock
            if let Some(wait) = wait_time {
                tokio::time::sleep(wait).await;
            }
        }
    }

    /// Returns the number of requests that can be made immediately.
    pub async fn available(&self) -> u32 {
        let mut state = self.inner.state.lock().await;
        let now = Instant::now();
        let cutoff = now - self.inner.window;

        // Remove expired timestamps
        while let Some(&ts) = state.timestamps.front() {
            if ts < cutoff {
                state.timestamps.pop_front();
            } else {
                break;
            }
        }

        self.inner
            .capacity
            .saturating_sub(state.timestamps.len() as u32)
    }

    /// Returns the configured capacity.
    pub fn capacity(&self) -> u32 {
        self.inner.capacity
    }

    /// Returns the configured window duration.
    pub fn window(&self) -> Duration {
        self.inner.window
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new(6000, Duration::from_secs(300))
    }
}
