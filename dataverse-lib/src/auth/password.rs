//! Password flow utilities (Resource Owner Password Credentials)

use async_trait::async_trait;
use chrono::Duration;
use chrono::Utc;
use serde::Deserialize;

use super::auto_refresh::AuthFlow;
use super::AccessToken;
use crate::error::AuthError;

/// Internal implementation handling both v1.0 and v2.0 endpoints.
struct PasswordFlowInner {
    client_id: String,
    client_secret: Option<String>,
    /// None means use "common" (auto-resolve from username domain)
    tenant: Option<String>,
    username: String,
    password: String,
    /// v1 uses `resource=`, v2 uses `scope=`
    use_v2: bool,
    http_client: reqwest::Client,
}

impl PasswordFlowInner {
    fn token_url(&self) -> String {
        let tenant = self.tenant.as_deref().unwrap_or("common");
        if self.use_v2 {
            format!(
                "https://login.microsoftonline.com/{}/oauth2/v2.0/token",
                tenant
            )
        } else {
            format!("https://login.windows.net/{}/oauth2/token", tenant)
        }
    }

    async fn authenticate(&self, resource: &str) -> Result<AccessToken, AuthError> {
        let token_url = self.token_url();

        let mut params = vec![
            ("grant_type", "password".to_string()),
            ("client_id", self.client_id.clone()),
            ("username", self.username.clone()),
            ("password", self.password.clone()),
        ];

        if let Some(secret) = &self.client_secret {
            params.push(("client_secret", secret.clone()));
        }

        if self.use_v2 {
            let scope = format!("{}/.default", resource.trim_end_matches('/'));
            params.push(("scope", scope));
        } else {
            params.push(("resource", resource.trim_end_matches('/').to_string()));
        }

        let response = self
            .http_client
            .post(&token_url)
            .form(&params)
            .send()
            .await?;

        self.handle_response(response).await
    }

    async fn refresh(&self, resource: &str, refresh_token: &str) -> Result<AccessToken, AuthError> {
        let token_url = self.token_url();

        let mut params = vec![
            ("grant_type", "refresh_token".to_string()),
            ("client_id", self.client_id.clone()),
            ("refresh_token", refresh_token.to_string()),
        ];

        if let Some(secret) = &self.client_secret {
            params.push(("client_secret", secret.clone()));
        }

        if self.use_v2 {
            let scope = format!("{}/.default", resource.trim_end_matches('/'));
            params.push(("scope", scope));
        } else {
            params.push(("resource", resource.trim_end_matches('/').to_string()));
        }

        let response = self
            .http_client
            .post(&token_url)
            .form(&params)
            .send()
            .await?;

        self.handle_response(response).await
    }

    async fn handle_response(&self, response: reqwest::Response) -> Result<AccessToken, AuthError> {
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

/// OAuth2 Resource Owner Password Credentials (ROPC) flow with a confidential client.
///
/// This flow uses the v1.0 endpoint with the "common" tenant, which automatically
/// resolves the tenant from the username's domain. It requires a client secret.
///
/// This matches the authentication pattern used in the `dynamics-api.sh` script.
///
/// # Example
///
/// ```ignore
/// use dataverse_lib::auth::PasswordFlow;
///
/// let flow = PasswordFlow::new(
///     "your-client-id",
///     "your-client-secret",
///     "user@example.com",
///     "password123",
/// );
///
/// let token = flow.authenticate("https://org.crm.dynamics.com").await?;
/// ```
#[derive(Debug, Clone)]
pub struct PasswordFlow {
    inner: PasswordFlowInner,
}

impl PasswordFlow {
    /// Creates a new password flow with client credentials.
    ///
    /// Uses the v1.0 Azure AD endpoint with "common" tenant, which automatically
    /// resolves the correct tenant from the username's domain.
    ///
    /// # Arguments
    ///
    /// * `client_id` - The Azure AD application (client) ID
    /// * `client_secret` - The Azure AD application client secret
    /// * `username` - The user's email or UPN
    /// * `password` - The user's password
    pub fn new(
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
        username: impl Into<String>,
        password: impl Into<String>,
    ) -> Self {
        Self {
            inner: PasswordFlowInner {
                client_id: client_id.into(),
                client_secret: Some(client_secret.into()),
                tenant: None, // "common"
                username: username.into(),
                password: password.into(),
                use_v2: false,
                http_client: reqwest::Client::new(),
            },
        }
    }

    /// Authenticates using username and password credentials.
    ///
    /// # Arguments
    ///
    /// * `resource` - The Dataverse environment URL (e.g., `https://org.crm.dynamics.com`)
    ///
    /// # Returns
    ///
    /// An access token that can be used for API calls, potentially with a refresh token.
    pub async fn authenticate(&self, resource: &str) -> Result<AccessToken, AuthError> {
        self.inner.authenticate(resource).await
    }

    /// Refreshes an access token using a refresh token.
    ///
    /// # Arguments
    ///
    /// * `resource` - The Dataverse environment URL
    /// * `refresh_token` - The refresh token from a previous authentication
    pub async fn refresh(
        &self,
        resource: &str,
        refresh_token: &str,
    ) -> Result<AccessToken, AuthError> {
        self.inner.refresh(resource, refresh_token).await
    }
}

/// OAuth2 Resource Owner Password Credentials (ROPC) flow with a public client.
///
/// This flow uses the v2.0 endpoint with an explicit tenant ID. It does not
/// require a client secret (public client).
///
/// # Example
///
/// ```ignore
/// use dataverse_lib::auth::PublicClientPasswordFlow;
///
/// let flow = PublicClientPasswordFlow::new(
///     "your-client-id",
///     "your-tenant-id",
///     "user@example.com",
///     "password123",
/// );
///
/// let token = flow.authenticate("https://org.crm.dynamics.com").await?;
/// ```
#[derive(Debug, Clone)]
pub struct PublicClientPasswordFlow {
    inner: PasswordFlowInner,
}

impl PublicClientPasswordFlow {
    /// Creates a new public client password flow.
    ///
    /// Uses the v2.0 Azure AD endpoint with an explicit tenant ID.
    ///
    /// # Arguments
    ///
    /// * `client_id` - The Azure AD application (client) ID
    /// * `tenant_id` - The Azure AD tenant ID or domain
    /// * `username` - The user's email or UPN
    /// * `password` - The user's password
    pub fn new(
        client_id: impl Into<String>,
        tenant_id: impl Into<String>,
        username: impl Into<String>,
        password: impl Into<String>,
    ) -> Self {
        Self {
            inner: PasswordFlowInner {
                client_id: client_id.into(),
                client_secret: None,
                tenant: Some(tenant_id.into()),
                username: username.into(),
                password: password.into(),
                use_v2: true,
                http_client: reqwest::Client::new(),
            },
        }
    }

    /// Authenticates using username and password credentials.
    ///
    /// # Arguments
    ///
    /// * `resource` - The Dataverse environment URL (e.g., `https://org.crm.dynamics.com`)
    ///
    /// # Returns
    ///
    /// An access token that can be used for API calls, potentially with a refresh token.
    pub async fn authenticate(&self, resource: &str) -> Result<AccessToken, AuthError> {
        self.inner.authenticate(resource).await
    }

    /// Refreshes an access token using a refresh token.
    ///
    /// # Arguments
    ///
    /// * `resource` - The Dataverse environment URL
    /// * `refresh_token` - The refresh token from a previous authentication
    pub async fn refresh(
        &self,
        resource: &str,
        refresh_token: &str,
    ) -> Result<AccessToken, AuthError> {
        self.inner.refresh(resource, refresh_token).await
    }
}

// PasswordFlowInner doesn't derive Clone/Debug because reqwest::Client is Clone
// but we want the public types to be Clone/Debug, so we implement manually
impl Clone for PasswordFlowInner {
    fn clone(&self) -> Self {
        Self {
            client_id: self.client_id.clone(),
            client_secret: self.client_secret.clone(),
            tenant: self.tenant.clone(),
            username: self.username.clone(),
            password: self.password.clone(),
            use_v2: self.use_v2,
            http_client: reqwest::Client::new(),
        }
    }
}

impl std::fmt::Debug for PasswordFlowInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PasswordFlowInner")
            .field("client_id", &self.client_id)
            .field(
                "client_secret",
                &self.client_secret.as_ref().map(|_| "[REDACTED]"),
            )
            .field("tenant", &self.tenant)
            .field("username", &self.username)
            .field("password", &"[REDACTED]")
            .field("use_v2", &self.use_v2)
            .finish()
    }
}

/// Token response from Azure AD.
#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default, deserialize_with = "deserialize_expires_in")]
    expires_in: Option<u64>,
    #[serde(default)]
    refresh_token: Option<String>,
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
    fn into_access_token(self) -> AccessToken {
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
struct ErrorResponse {
    error: String,
    error_description: Option<String>,
}

/// Maps Azure AD error codes to AuthError variants.
fn map_error_response(error: ErrorResponse) -> AuthError {
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
// AuthFlow implementations
// =============================================================================

#[async_trait]
impl AuthFlow for PasswordFlow {
    async fn authenticate(&self, resource: &str) -> Result<AccessToken, AuthError> {
        self.authenticate(resource).await
    }

    async fn refresh(&self, resource: &str, refresh_token: &str) -> Result<AccessToken, AuthError> {
        self.refresh(resource, refresh_token).await
    }
}

#[async_trait]
impl AuthFlow for PublicClientPasswordFlow {
    async fn authenticate(&self, resource: &str) -> Result<AccessToken, AuthError> {
        self.authenticate(resource).await
    }

    async fn refresh(&self, resource: &str, refresh_token: &str) -> Result<AccessToken, AuthError> {
        self.refresh(resource, refresh_token).await
    }
}
