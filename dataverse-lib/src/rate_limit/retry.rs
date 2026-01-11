//! Retry configuration for automatic request retry.

use std::time::Duration;

/// Configuration for automatic retry behavior.
///
/// Controls how the client handles transient failures such as rate limiting (429),
/// server errors (5xx), and network errors.
///
/// # Example
///
/// ```
/// use std::time::Duration;
/// use dataverse_lib::rate_limit::RetryConfig;
///
/// // Default configuration
/// let config = RetryConfig::default();
///
/// // Custom configuration
/// let custom = RetryConfig::default()
///     .max_retries(5)
///     .initial_delay(Duration::from_millis(500))
///     .max_delay(Duration::from_secs(60));
///
/// // Disable all retries
/// let no_retry = RetryConfig::no_retry();
/// ```
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts.
    pub max_retries: u32,
    /// Initial delay between retries (doubles each attempt).
    pub initial_delay: Duration,
    /// Maximum delay between retries.
    pub max_delay: Duration,
    /// Whether to retry on HTTP 429 (rate limited).
    pub retry_on_429: bool,
    /// Whether to retry on HTTP 5xx (server errors).
    pub retry_on_5xx: bool,
    /// Whether to retry on network errors.
    pub retry_on_network: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(30),
            retry_on_429: true,
            retry_on_5xx: true,
            retry_on_network: true,
        }
    }
}

impl RetryConfig {
    /// Creates a config with all retries disabled.
    pub fn no_retry() -> Self {
        Self {
            max_retries: 0,
            retry_on_429: false,
            retry_on_5xx: false,
            retry_on_network: false,
            ..Default::default()
        }
    }

    /// Sets the maximum number of retries.
    pub fn max_retries(mut self, n: u32) -> Self {
        self.max_retries = n;
        self
    }

    /// Sets the initial delay between retries.
    pub fn initial_delay(mut self, delay: Duration) -> Self {
        self.initial_delay = delay;
        self
    }

    /// Sets the maximum delay between retries.
    pub fn max_delay(mut self, delay: Duration) -> Self {
        self.max_delay = delay;
        self
    }

    /// Enables or disables retry on HTTP 429.
    pub fn retry_on_429(mut self, enabled: bool) -> Self {
        self.retry_on_429 = enabled;
        self
    }

    /// Enables or disables retry on HTTP 5xx.
    pub fn retry_on_5xx(mut self, enabled: bool) -> Self {
        self.retry_on_5xx = enabled;
        self
    }

    /// Enables or disables retry on network errors.
    pub fn retry_on_network(mut self, enabled: bool) -> Self {
        self.retry_on_network = enabled;
        self
    }
}
