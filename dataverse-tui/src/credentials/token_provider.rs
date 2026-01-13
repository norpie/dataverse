//! Token provider backed by credentials storage.

use async_trait::async_trait;
use chrono::Duration;
use dataverse_lib::auth::AccessToken;
use dataverse_lib::auth::BrowserFlow;
use dataverse_lib::auth::DeviceCodeFlow;
use dataverse_lib::auth::PasswordFlow;
use dataverse_lib::auth::PublicClientPasswordFlow;
use dataverse_lib::auth::TokenProvider;
use dataverse_lib::error::AuthError;

use super::models::Account;
use super::models::AuthType;
use super::models::CachedTokens;
use super::models::Environment;
use super::CredentialsError;
use super::CredentialsProvider;

/// Buffer time before expiry to trigger refresh (5 minutes).
const REFRESH_BUFFER_SECS: i64 = 300;

/// Token provider that reads/writes tokens from credentials storage.
///
/// This provider:
/// - Reads cached tokens from the database
/// - Returns valid tokens directly
/// - Refreshes expired tokens using the appropriate auth flow
/// - Persists refreshed tokens back to the database
/// - Returns `ReauthRequired` error when refresh fails
pub struct StoredTokenProvider {
    credentials: CredentialsProvider,
    account_id: i64,
    environment_id: i64,
    resource: String,
    // Cached account data for refresh
    auth_type: AuthType,
    client_id: String,
    tenant_id: Option<String>,
    client_secret: Option<String>,
}

impl StoredTokenProvider {
    /// Create a new stored token provider.
    pub fn new(credentials: CredentialsProvider, account: Account, environment: Environment) -> Self {
        Self {
            credentials,
            account_id: account.id,
            environment_id: environment.id,
            resource: environment.url,
            auth_type: account.auth_type,
            client_id: account.client_id,
            tenant_id: account.tenant_id,
            client_secret: account.client_secret,
        }
    }

    /// Refresh the token using the appropriate auth flow.
    async fn refresh_token(&self, refresh_token: &str) -> Result<AccessToken, AuthError> {
        match self.auth_type {
            AuthType::Browser => {
                let tenant_id = self.tenant_id.as_deref().unwrap_or("common");
                let flow = BrowserFlow::new(&self.client_id, tenant_id);
                flow.refresh(&self.resource, refresh_token).await
            }
            AuthType::DeviceCode => {
                let tenant_id = self.tenant_id.as_deref().unwrap_or("common");
                let flow = DeviceCodeFlow::new(&self.client_id, tenant_id);
                flow.refresh(&self.resource, refresh_token).await
            }
            AuthType::Password => {
                let client_secret = self.client_secret.as_deref().unwrap_or("");
                let flow = PasswordFlow::for_refresh(&self.client_id, client_secret);
                flow.refresh(&self.resource, refresh_token).await
            }
            AuthType::PublicPassword => {
                let tenant_id = self.tenant_id.as_deref().unwrap_or("common");
                let flow = PublicClientPasswordFlow::for_refresh(&self.client_id, tenant_id);
                flow.refresh(&self.resource, refresh_token).await
            }
        }
    }

    /// Convert AccessToken to CachedTokens for storage.
    fn to_cached_tokens(token: &AccessToken) -> CachedTokens {
        CachedTokens {
            access_token: token.access_token.clone(),
            expires_at: token.expires_at,
            refresh_token: token.refresh_token.clone(),
        }
    }

    /// Convert CachedTokens to AccessToken.
    fn to_access_token(tokens: &CachedTokens) -> AccessToken {
        match (&tokens.expires_at, &tokens.refresh_token) {
            (Some(expires_at), Some(refresh_token)) => {
                AccessToken::with_refresh(&tokens.access_token, Some(*expires_at), refresh_token)
            }
            (Some(expires_at), None) => {
                AccessToken::with_expiry(&tokens.access_token, *expires_at)
            }
            (None, Some(refresh_token)) => {
                AccessToken::with_refresh(&tokens.access_token, None, refresh_token)
            }
            (None, None) => AccessToken::new(&tokens.access_token),
        }
    }

    /// Create a ReauthRequired error as AuthError.
    fn reauth_required(&self) -> AuthError {
        // We need to return an AuthError, but we want to signal reauth is needed.
        // Using TokenExpired as the signal since it indicates the token can't be used.
        AuthError::TokenExpired {
            message: format!(
                "Re-authentication required for account {} on environment {}",
                self.account_id, self.environment_id
            ),
        }
    }
}

#[async_trait]
impl TokenProvider for StoredTokenProvider {
    async fn get_token(&self, _resource: &str) -> Result<AccessToken, AuthError> {
        // Read cached tokens from database
        let cached = self
            .credentials
            .get_tokens(self.account_id, self.environment_id)
            .await
            .map_err(|e| AuthError::Parse(format!("Failed to read tokens: {}", e)))?;

        let Some(tokens) = cached else {
            // No cached tokens - need initial auth
            return Err(self.reauth_required());
        };

        // Check if token is still valid (with buffer)
        if !tokens.is_expired_within(REFRESH_BUFFER_SECS) {
            return Ok(Self::to_access_token(&tokens));
        }

        // Token expired or expiring soon - try to refresh
        let Some(refresh_token) = &tokens.refresh_token else {
            // No refresh token - need re-auth
            return Err(self.reauth_required());
        };

        // Attempt refresh
        let new_token = match self.refresh_token(refresh_token).await {
            Ok(token) => token,
            Err(_) => {
                // Refresh failed - need re-auth
                return Err(self.reauth_required());
            }
        };

        // Persist new tokens
        let new_cached = Self::to_cached_tokens(&new_token);
        self.credentials
            .save_tokens(self.account_id, self.environment_id, &new_cached)
            .await
            .map_err(|e| AuthError::Parse(format!("Failed to save tokens: {}", e)))?;

        Ok(new_token)
    }
}
