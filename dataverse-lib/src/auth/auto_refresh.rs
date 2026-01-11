//! Automatic token refresh handling.

use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::RwLock;

use super::AccessToken;
use super::TokenProvider;
use crate::error::AuthError;

/// Trait for authentication flows that support token refresh.
///
/// Implement this trait for any authentication flow that can obtain
/// and refresh tokens. Used with [`AutoRefreshTokenProvider`] for
/// automatic token management.
#[async_trait]
pub trait AuthFlow: Send + Sync {
    /// Authenticates and obtains a new access token.
    ///
    /// # Arguments
    ///
    /// * `resource` - The Dataverse environment URL (e.g., `https://org.crm.dynamics.com`)
    async fn authenticate(&self, resource: &str) -> Result<AccessToken, AuthError>;

    /// Refreshes an access token using a refresh token.
    ///
    /// # Arguments
    ///
    /// * `resource` - The Dataverse environment URL
    /// * `refresh_token` - The refresh token from a previous authentication
    async fn refresh(&self, resource: &str, refresh_token: &str) -> Result<AccessToken, AuthError>;
}

/// A token provider that automatically caches and refreshes tokens.
///
/// Wraps any [`AuthFlow`] implementation and provides automatic token
/// management:
/// - Caches the current token
/// - Returns cached token if still valid
/// - Refreshes using refresh_token when token is expired or expiring soon
/// - Falls back to full re-authentication if refresh fails
///
/// # Example
///
/// ```ignore
/// use dataverse_lib::auth::{PasswordFlow, AutoRefreshTokenProvider};
/// use dataverse_lib::DataverseClient;
///
/// let flow = PasswordFlow::new(client_id, client_secret, username, password);
/// let provider = AutoRefreshTokenProvider::new(flow);
///
/// let client = DataverseClient::builder()
///     .url("https://org.crm.dynamics.com")
///     .token_provider(provider)
///     .build();
///
/// // Tokens are automatically managed - no manual refresh needed
/// client.create(entity, record).await?;
/// ```
pub struct AutoRefreshTokenProvider<F> {
    flow: F,
    token: RwLock<Option<AccessToken>>,
    /// Refresh this many seconds before actual expiry
    refresh_buffer: Duration,
}

impl<F: AuthFlow> AutoRefreshTokenProvider<F> {
    /// Creates a new auto-refresh token provider.
    ///
    /// Uses a default refresh buffer of 5 minutes (tokens are refreshed
    /// 5 minutes before they expire).
    pub fn new(flow: F) -> Self {
        Self {
            flow,
            token: RwLock::new(None),
            refresh_buffer: Duration::from_secs(300), // 5 minutes
        }
    }

    /// Creates a new auto-refresh token provider with a custom refresh buffer.
    ///
    /// # Arguments
    ///
    /// * `flow` - The authentication flow to use
    /// * `refresh_buffer` - How long before expiry to refresh the token
    pub fn with_refresh_buffer(flow: F, refresh_buffer: Duration) -> Self {
        Self {
            flow,
            token: RwLock::new(None),
            refresh_buffer,
        }
    }

    /// Clears the cached token, forcing re-authentication on next request.
    pub async fn clear_token(&self) {
        let mut token = self.token.write().await;
        *token = None;
    }
}

#[async_trait]
impl<F: AuthFlow> TokenProvider for AutoRefreshTokenProvider<F> {
    async fn get_token(&self, resource: &str) -> Result<AccessToken, AuthError> {
        // Fast path: check if we have a valid cached token
        {
            let token_guard = self.token.read().await;
            if let Some(ref token) = *token_guard {
                let buffer = chrono::Duration::from_std(self.refresh_buffer)
                    .unwrap_or(chrono::Duration::zero());
                if !token.expires_within(buffer) {
                    return Ok(token.clone());
                }
            }
        }

        // Slow path: need to refresh or authenticate
        let mut token_guard = self.token.write().await;

        // Double-check after acquiring write lock (another task may have refreshed)
        if let Some(ref token) = *token_guard {
            let buffer =
                chrono::Duration::from_std(self.refresh_buffer).unwrap_or(chrono::Duration::zero());
            if !token.expires_within(buffer) {
                return Ok(token.clone());
            }
        }

        // Try to refresh if we have a refresh token
        let new_token = if let Some(ref old_token) = *token_guard {
            if let Some(ref refresh_token) = old_token.refresh_token {
                match self.flow.refresh(resource, refresh_token).await {
                    Ok(token) => token,
                    Err(_) => {
                        // Refresh failed, fall back to full authentication
                        self.flow.authenticate(resource).await?
                    }
                }
            } else {
                // No refresh token, must re-authenticate
                self.flow.authenticate(resource).await?
            }
        } else {
            // No cached token, initial authentication
            self.flow.authenticate(resource).await?
        };

        *token_guard = Some(new_token.clone());
        Ok(new_token)
    }
}
