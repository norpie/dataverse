//! Device code flow utilities

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::DateTime;
use chrono::Utc;
use serde::Deserialize;
use tokio_util::sync::CancellationToken;

use super::auto_refresh::AuthFlow;
use super::common::device_code_url_v2;
use super::common::scope_from_resource;
use super::common::DeviceCodeTokenResult;
use super::common::ErrorResponse;
use super::common::TokenExchange;
use super::AccessToken;
use crate::error::AuthError;

// =============================================================================
// DeviceCodeFlow
// =============================================================================

/// Device code authentication flow.
///
/// This flow is ideal for devices without a browser or with limited input
/// capabilities. The user authenticates on a separate device by visiting
/// a URL and entering a code.
///
/// # Example
///
/// ```ignore
/// use dataverse_lib::auth::DeviceCodeFlow;
///
/// let flow = DeviceCodeFlow::new("your-client-id", "your-tenant-id");
/// let pending = flow.start("https://org.crm.dynamics.com").await?;
///
/// println!("Go to: {}", pending.info.verification_url);
/// println!("Enter code: {}", pending.info.user_code);
///
/// let token = pending.wait().await?;
/// ```
#[derive(Clone)]
pub struct DeviceCodeFlow {
    inner: Arc<DeviceCodeFlowInner>,
}

struct DeviceCodeFlowInner {
    client_id: String,
    tenant_id: String,
    http_client: reqwest::Client,
}

impl DeviceCodeFlow {
    /// Creates a new device code flow.
    ///
    /// # Arguments
    ///
    /// * `client_id` - The Azure AD application (client) ID
    /// * `tenant_id` - The Azure AD tenant ID or domain
    pub fn new(client_id: impl Into<String>, tenant_id: impl Into<String>) -> Self {
        Self {
            inner: Arc::new(DeviceCodeFlowInner {
                client_id: client_id.into(),
                tenant_id: tenant_id.into(),
                http_client: reqwest::Client::new(),
            }),
        }
    }

    /// Start the device code flow.
    ///
    /// Returns pending auth info that should be displayed to the user.
    /// The user must visit `verification_url` and enter `user_code` to authenticate.
    pub async fn start(&self, resource: &str) -> Result<PendingDeviceAuth, AuthError> {
        let device_code_url = device_code_url_v2(&self.inner.tenant_id);
        let scope = scope_from_resource(resource);

        let params = [("client_id", &self.inner.client_id), ("scope", &scope)];

        let response = self
            .inner
            .http_client
            .post(&device_code_url)
            .form(&params)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_response: ErrorResponse =
                response.json().await.unwrap_or_else(|_| ErrorResponse {
                    error: "unknown".to_string(),
                    error_description: None,
                });
            return Err(super::common::map_error_response(error_response));
        }

        let device_response: DeviceCodeResponse = response.json().await.map_err(|e| {
            AuthError::Parse(format!("Failed to parse device code response: {}", e))
        })?;

        let expires_at = Utc::now()
            + chrono::Duration::seconds(device_response.expires_in.unwrap_or(900) as i64);

        let interval = Duration::from_secs(device_response.interval.unwrap_or(5));

        Ok(PendingDeviceAuth {
            info: DeviceCodeInfo {
                user_code: device_response.user_code,
                verification_url: device_response.verification_uri,
                expires_at,
                message: device_response.message,
            },
            interval,
            flow: self.clone(),
            device_code: device_response.device_code,
            resource: resource.to_string(),
        })
    }

    /// Refresh an access token.
    pub async fn refresh(
        &self,
        resource: &str,
        refresh_token: &str,
    ) -> Result<AccessToken, AuthError> {
        let exchange = TokenExchange {
            http_client: &self.inner.http_client,
            client_id: &self.inner.client_id,
            tenant_id: &self.inner.tenant_id,
            resource,
        };
        exchange.refresh(refresh_token).await
    }
}

impl std::fmt::Debug for DeviceCodeFlow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeviceCodeFlow")
            .field("client_id", &self.inner.client_id)
            .field("tenant_id", &self.inner.tenant_id)
            .finish()
    }
}

// =============================================================================
// DeviceCodeInfo
// =============================================================================

/// Information displayed to the user during device code authentication.
#[derive(Debug, Clone)]
pub struct DeviceCodeInfo {
    /// The code the user must enter (e.g., "ABCD-EFGH")
    pub user_code: String,
    /// URL where the user enters the code (e.g., "https://microsoft.com/devicelogin")
    pub verification_url: String,
    /// When this code expires
    pub expires_at: DateTime<Utc>,
    /// Human-readable message (e.g., "To sign in, use a web browser to open...")
    pub message: String,
}

// =============================================================================
// PendingDeviceAuth
// =============================================================================

/// A pending device code authentication.
///
/// Display `info` to the user, then call `wait()` or use `poll()` for manual control.
pub struct PendingDeviceAuth {
    /// Information to display to the user
    pub info: DeviceCodeInfo,
    /// Recommended polling interval
    pub interval: Duration,
    // Internal fields
    flow: DeviceCodeFlow,
    device_code: String,
    resource: String,
}

impl PendingDeviceAuth {
    /// Poll once for authentication completion.
    ///
    /// Returns `Pending` if user hasn't completed auth yet.
    /// Call this at `self.interval` intervals.
    pub async fn poll(&self) -> Result<PollResult, AuthError> {
        // Check expiry first
        if Utc::now() >= self.info.expires_at {
            return Ok(PollResult::Expired);
        }

        let exchange = TokenExchange {
            http_client: &self.flow.inner.http_client,
            client_id: &self.flow.inner.client_id,
            tenant_id: &self.flow.inner.tenant_id,
            resource: &self.resource,
        };

        match exchange.device_code(&self.device_code).await? {
            DeviceCodeTokenResult::Success(token) => Ok(PollResult::Complete(token)),
            DeviceCodeTokenResult::Pending => Ok(PollResult::Pending),
            DeviceCodeTokenResult::Expired => Ok(PollResult::Expired),
            DeviceCodeTokenResult::Declined => Err(AuthError::DeviceCodeDeclined),
        }
    }

    /// Wait for authentication to complete, polling automatically.
    ///
    /// This will poll at the recommended interval until the user completes
    /// authentication, the code expires, or an error occurs.
    pub async fn wait(self) -> Result<AccessToken, AuthError> {
        loop {
            match self.poll().await? {
                PollResult::Complete(token) => return Ok(token),
                PollResult::Expired => return Err(AuthError::DeviceCodeExpired),
                PollResult::Pending => {
                    tokio::time::sleep(self.interval).await;
                }
            }
        }
    }

    /// Wait for authentication with cancellation support.
    ///
    /// Returns `AuthError::DeviceCodeExpired` if cancelled (since the flow
    /// cannot continue after cancellation).
    pub async fn wait_with_cancel(
        self,
        cancel: CancellationToken,
    ) -> Result<AccessToken, AuthError> {
        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    return Err(AuthError::DeviceCodeExpired);
                }
                result = self.poll() => {
                    match result? {
                        PollResult::Complete(token) => return Ok(token),
                        PollResult::Expired => return Err(AuthError::DeviceCodeExpired),
                        PollResult::Pending => {
                            // Sleep with cancellation support
                            tokio::select! {
                                _ = cancel.cancelled() => {
                                    return Err(AuthError::DeviceCodeExpired);
                                }
                                _ = tokio::time::sleep(self.interval) => {}
                            }
                        }
                    }
                }
            }
        }
    }
}

impl std::fmt::Debug for PendingDeviceAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PendingDeviceAuth")
            .field("info", &self.info)
            .field("interval", &self.interval)
            .field("resource", &self.resource)
            .finish_non_exhaustive()
    }
}

// =============================================================================
// PollResult
// =============================================================================

/// Result of a single poll attempt.
#[derive(Debug)]
pub enum PollResult {
    /// User hasn't completed authentication yet
    Pending,
    /// Authentication successful
    Complete(AccessToken),
    /// Device code expired
    Expired,
}

// =============================================================================
// Internal Types
// =============================================================================

/// Response from the device code endpoint.
#[derive(Debug, Deserialize)]
struct DeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    #[serde(default)]
    expires_in: Option<u64>,
    #[serde(default)]
    interval: Option<u64>,
    #[serde(default)]
    message: String,
}

// =============================================================================
// AuthFlow Implementation
// =============================================================================

#[async_trait]
impl AuthFlow for DeviceCodeFlow {
    /// Authenticate using device code flow.
    ///
    /// **Note:** This method blocks until the user completes authentication.
    /// For better UX in interactive applications, use `start()` and `wait()`
    /// directly so you can display the user code and handle cancellation.
    async fn authenticate(&self, resource: &str) -> Result<AccessToken, AuthError> {
        let pending = self.start(resource).await?;
        pending.wait().await
    }

    async fn refresh(&self, resource: &str, refresh_token: &str) -> Result<AccessToken, AuthError> {
        self.refresh(resource, refresh_token).await
    }
}
