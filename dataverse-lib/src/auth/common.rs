//! Shared authentication utilities

use chrono::Duration;
use chrono::Utc;
use serde::Deserialize;

use super::AccessToken;
use crate::error::AuthError;

// =============================================================================
// URL Helpers
// =============================================================================

/// Build v2 token endpoint URL.
pub(crate) fn token_url_v2(tenant_id: &str) -> String {
    format!(
        "https://login.microsoftonline.com/{}/oauth2/v2.0/token",
        tenant_id
    )
}

/// Build v2 device code endpoint URL.
pub(crate) fn device_code_url_v2(tenant_id: &str) -> String {
    format!(
        "https://login.microsoftonline.com/{}/oauth2/v2.0/devicecode",
        tenant_id
    )
}

/// Build v2 authorize endpoint URL.
pub(crate) fn authorize_url_v2(tenant_id: &str) -> String {
    format!(
        "https://login.microsoftonline.com/{}/oauth2/v2.0/authorize",
        tenant_id
    )
}

/// Build scope string from resource URL.
pub(crate) fn scope_from_resource(resource: &str) -> String {
    format!("{}/.default offline_access", resource.trim_end_matches('/'))
}

// =============================================================================
// Token Response Parsing
// =============================================================================

/// Token response from Azure AD.
#[derive(Debug, Deserialize)]
pub(crate) struct TokenResponse {
    pub access_token: String,
    #[serde(default, deserialize_with = "deserialize_expires_in")]
    pub expires_in: Option<u64>,
    #[serde(default)]
    pub refresh_token: Option<String>,
}

/// Deserializes `expires_in` which can be either a number or a string.
fn deserialize_expires_in<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StringOrNumber {
        String(String),
        Number(u64),
    }

    match Option::<StringOrNumber>::deserialize(deserializer)? {
        None => Ok(None),
        Some(StringOrNumber::Number(n)) => Ok(Some(n)),
        Some(StringOrNumber::String(s)) => s
            .parse::<u64>()
            .map(Some)
            .map_err(|_| D::Error::custom(format!("invalid expires_in value: {}", s))),
    }
}

impl TokenResponse {
    pub fn into_access_token(self) -> AccessToken {
        let expires_at = self
            .expires_in
            .map(|secs| Utc::now() + Duration::seconds(secs as i64));

        match self.refresh_token {
            Some(refresh) => AccessToken::with_refresh(self.access_token, expires_at, refresh),
            None => match expires_at {
                Some(exp) => AccessToken::with_expiry(self.access_token, exp),
                None => AccessToken::new(self.access_token),
            },
        }
    }
}

/// Error response from Azure AD.
#[derive(Debug, Deserialize)]
pub(crate) struct ErrorResponse {
    pub error: String,
    pub error_description: Option<String>,
}

/// Maps Azure AD error codes to AuthError variants.
pub(crate) fn map_error_response(error: ErrorResponse) -> AuthError {
    let description = error
        .error_description
        .unwrap_or_else(|| error.error.clone());

    match error.error.as_str() {
        "invalid_grant" => {
            if description.contains("AADSTS50126") {
                // Invalid username or password
                AuthError::InvalidCredentials
            } else if description.contains("AADSTS700082") || description.contains("AADSTS50173") {
                // Refresh token expired
                AuthError::TokenExpired {
                    message: description,
                }
            } else {
                AuthError::InvalidCredentials
            }
        }
        "invalid_client" => AuthError::InvalidClient {
            client_id: description,
        },
        "unauthorized_client" => AuthError::InvalidClient {
            client_id: description,
        },
        "invalid_request" => AuthError::Parse(description),
        _ => {
            if description.contains("AADSTS90002") || description.contains("AADSTS90014") {
                // Tenant not found
                AuthError::InvalidTenant {
                    tenant: description,
                }
            } else {
                AuthError::Parse(description)
            }
        }
    }
}

// =============================================================================
// Token Exchange
// =============================================================================

/// Result of a device code token exchange attempt.
pub(crate) enum DeviceCodeTokenResult {
    /// Authentication successful
    Success(AccessToken),
    /// User hasn't completed authentication yet
    Pending,
    /// Device code expired
    Expired,
    /// User declined authentication
    Declined,
}

/// Internal helper for token endpoint calls.
pub(crate) struct TokenExchange<'a> {
    pub http_client: &'a reqwest::Client,
    pub client_id: &'a str,
    pub tenant_id: &'a str,
    pub resource: &'a str,
}

impl TokenExchange<'_> {
    /// Exchange device code for token.
    pub async fn device_code(&self, device_code: &str) -> Result<DeviceCodeTokenResult, AuthError> {
        let token_url = token_url_v2(self.tenant_id);
        let scope = scope_from_resource(self.resource);

        let params = [
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ("client_id", self.client_id),
            ("device_code", device_code),
            ("scope", &scope),
        ];

        let response = self
            .http_client
            .post(&token_url)
            .form(&params)
            .send()
            .await?;

        if response.status().is_success() {
            let token_response: TokenResponse = response.json().await?;
            return Ok(DeviceCodeTokenResult::Success(
                token_response.into_access_token(),
            ));
        }

        // Parse error response
        let error_response: ErrorResponse =
            response.json().await.unwrap_or_else(|_| ErrorResponse {
                error: "unknown".to_string(),
                error_description: None,
            });

        // Check for device code specific errors
        match error_response.error.as_str() {
            "authorization_pending" => Ok(DeviceCodeTokenResult::Pending),
            "slow_down" => Ok(DeviceCodeTokenResult::Pending), // Caller should increase interval
            "expired_token" => Ok(DeviceCodeTokenResult::Expired),
            "authorization_declined" => Ok(DeviceCodeTokenResult::Declined),
            _ => Err(map_error_response(error_response)),
        }
    }

    /// Exchange authorization code for token (PKCE flow).
    pub async fn authorization_code(
        &self,
        code: &str,
        redirect_uri: &str,
        code_verifier: &str,
    ) -> Result<AccessToken, AuthError> {
        let token_url = token_url_v2(self.tenant_id);
        let scope = scope_from_resource(self.resource);

        let params = [
            ("grant_type", "authorization_code"),
            ("client_id", self.client_id),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("code_verifier", code_verifier),
            ("scope", &scope),
        ];

        let response = self
            .http_client
            .post(&token_url)
            .form(&params)
            .send()
            .await?;

        self.handle_token_response(response).await
    }

    /// Refresh an access token.
    pub async fn refresh(&self, refresh_token: &str) -> Result<AccessToken, AuthError> {
        let token_url = token_url_v2(self.tenant_id);
        let scope = scope_from_resource(self.resource);

        let params = [
            ("grant_type", "refresh_token"),
            ("client_id", self.client_id),
            ("refresh_token", refresh_token),
            ("scope", &scope),
        ];

        let response = self
            .http_client
            .post(&token_url)
            .form(&params)
            .send()
            .await?;

        self.handle_token_response(response).await
    }

    async fn handle_token_response(
        &self,
        response: reqwest::Response,
    ) -> Result<AccessToken, AuthError> {
        if response.status().is_success() {
            let token_response: TokenResponse = response.json().await?;
            Ok(token_response.into_access_token())
        } else {
            let error_response: ErrorResponse =
                response.json().await.unwrap_or_else(|_| ErrorResponse {
                    error: "unknown".to_string(),
                    error_description: None,
                });
            Err(map_error_response(error_response))
        }
    }
}
