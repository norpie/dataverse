//! TokenProvider trait and AccessToken

use async_trait::async_trait;
use chrono::DateTime;
use chrono::Utc;

use crate::error::AuthError;

/// An OAuth2 access token with optional expiration and refresh token.
///
/// This struct represents the result of a successful authentication flow.
/// It contains the access token needed to make API calls, along with
/// optional metadata about expiration and refresh capabilities.
#[derive(Debug, Clone)]
pub struct AccessToken {
    /// The bearer token used for API authentication.
    pub access_token: String,
    /// When the token expires, if known.
    pub expires_at: Option<DateTime<Utc>>,
    /// Refresh token for obtaining new access tokens without re-authentication.
    pub refresh_token: Option<String>,
}

impl AccessToken {
    /// Creates a new access token with just the token string.
    pub fn new(access_token: impl Into<String>) -> Self {
        Self {
            access_token: access_token.into(),
            expires_at: None,
            refresh_token: None,
        }
    }

    /// Creates a new access token with expiration time.
    pub fn with_expiry(access_token: impl Into<String>, expires_at: DateTime<Utc>) -> Self {
        Self {
            access_token: access_token.into(),
            expires_at: Some(expires_at),
            refresh_token: None,
        }
    }

    /// Creates a new access token with expiration and refresh token.
    pub fn with_refresh(
        access_token: impl Into<String>,
        expires_at: Option<DateTime<Utc>>,
        refresh_token: impl Into<String>,
    ) -> Self {
        Self {
            access_token: access_token.into(),
            expires_at,
            refresh_token: Some(refresh_token.into()),
        }
    }

    /// Returns `true` if the token has expired.
    ///
    /// Returns `false` if expiration time is unknown.
    pub fn is_expired(&self) -> bool {
        self.expires_at.is_some_and(|exp| Utc::now() >= exp)
    }

    /// Returns `true` if the token will expire within the given duration.
    ///
    /// Returns `false` if expiration time is unknown.
    pub fn expires_within(&self, duration: chrono::Duration) -> bool {
        self.expires_at
            .is_some_and(|exp| Utc::now() + duration >= exp)
    }

    /// Returns `true` if a refresh token is available.
    pub fn can_refresh(&self) -> bool {
        self.refresh_token.is_some()
    }

    /// Returns the token as a bearer authorization header value.
    pub fn as_bearer(&self) -> String {
        format!("Bearer {}", self.access_token)
    }
}

/// Trait for providing access tokens to the Dataverse client.
///
/// Implementors of this trait are responsible for:
/// - Obtaining initial tokens (via any authentication flow)
/// - Caching tokens to avoid unnecessary re-authentication
/// - Refreshing tokens when they expire
/// - Persisting tokens if desired (e.g., to disk or secure storage)
///
/// The client calls `get_token` before each API request. Implementations
/// should return cached tokens when valid and handle refresh/re-auth
/// transparently.
///
/// # Example
///
/// ```ignore
/// use async_trait::async_trait;
/// use dataverse_lib::auth::{TokenProvider, AccessToken};
/// use dataverse_lib::error::AuthError;
///
/// struct MyTokenProvider {
///     token: std::sync::RwLock<Option<AccessToken>>,
/// }
///
/// #[async_trait]
/// impl TokenProvider for MyTokenProvider {
///     async fn get_token(&self, resource: &str) -> Result<AccessToken, AuthError> {
///         // Return cached token if valid, otherwise refresh or re-authenticate
///         let guard = self.token.read().unwrap();
///         if let Some(token) = &*guard {
///             if !token.is_expired() {
///                 return Ok(token.clone());
///             }
///         }
///         drop(guard);
///
///         // Acquire new token...
///         todo!("Implement token acquisition")
///     }
/// }
/// ```
#[async_trait]
pub trait TokenProvider: Send + Sync {
    /// Gets an access token for the specified resource.
    ///
    /// The `resource` parameter is the Dataverse environment URL
    /// (e.g., `https://org.crm.dynamics.com`).
    ///
    /// Implementations should:
    /// - Return a cached token if still valid
    /// - Refresh the token if expired but refresh token is available
    /// - Re-authenticate if no valid token or refresh token exists
    async fn get_token(&self, resource: &str) -> Result<AccessToken, AuthError>;
}

/// A simple token provider that always returns the same static token.
///
/// Useful for testing or when you have a long-lived token that doesn't
/// need refresh logic.
///
/// # Example
///
/// ```
/// use dataverse_lib::auth::{StaticTokenProvider, AccessToken};
///
/// let provider = StaticTokenProvider::new("my-access-token");
/// ```
#[derive(Debug, Clone)]
pub struct StaticTokenProvider {
    token: AccessToken,
}

impl StaticTokenProvider {
    /// Creates a new static token provider with the given access token.
    pub fn new(access_token: impl Into<String>) -> Self {
        Self {
            token: AccessToken::new(access_token),
        }
    }

    /// Creates a new static token provider from an existing AccessToken.
    pub fn from_token(token: AccessToken) -> Self {
        Self { token }
    }
}

#[async_trait]
impl TokenProvider for StaticTokenProvider {
    async fn get_token(&self, _resource: &str) -> Result<AccessToken, AuthError> {
        Ok(self.token.clone())
    }
}
