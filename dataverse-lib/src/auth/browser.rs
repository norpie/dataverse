//! Authorization code + PKCE flow

use std::collections::HashMap;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;

use async_trait::async_trait;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use http_body_util::Full;
use hyper::body::Bytes;
use hyper::body::Incoming;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::Request;
use hyper::Response;
use hyper_util::rt::TokioIo;
use rand::Rng;
use sha2::Digest;
use sha2::Sha256;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

use super::auto_refresh::AuthFlow;
use super::common::authorize_url_v2;
use super::common::scope_from_resource;
use super::common::TokenExchange;
use super::AccessToken;
use crate::error::AuthError;

// =============================================================================
// PKCE Utilities
// =============================================================================

/// Generate a random code verifier (43-128 chars, [A-Za-z0-9-._~])
fn generate_code_verifier() -> String {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~";
    let mut rng = rand::rng();
    (0..128)
        .map(|_| {
            let idx = rng.random_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

/// Generate S256 code challenge from verifier
fn generate_code_challenge(verifier: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    let hash = hasher.finalize();
    URL_SAFE_NO_PAD.encode(hash)
}

/// Generate random state parameter for CSRF protection
fn generate_state() -> String {
    let mut rng = rand::rng();
    let bytes: [u8; 32] = rng.random();
    URL_SAFE_NO_PAD.encode(bytes)
}

// =============================================================================
// BrowserFlow
// =============================================================================

/// Browser-based authentication flow using Authorization Code + PKCE.
///
/// This flow opens the user's browser for authentication and listens
/// for the OAuth callback on a local HTTP server.
///
/// # Example
///
/// ```ignore
/// use dataverse_lib::auth::BrowserFlow;
///
/// let flow = BrowserFlow::new("your-client-id", "your-tenant-id");
/// let pending = flow.start("https://org.crm.dynamics.com").await?;
///
/// // Open browser (e.g., using the `open` crate)
/// open::that(&pending.auth_url)?;
///
/// // Wait for user to complete authentication
/// let token = pending.wait().await?;
/// ```
#[derive(Clone)]
pub struct BrowserFlow {
    inner: Arc<BrowserFlowInner>,
}

struct BrowserFlowInner {
    client_id: String,
    tenant_id: String,
    redirect_port: Option<u16>,
    http_client: reqwest::Client,
}

impl BrowserFlow {
    /// Creates a new browser authentication flow.
    ///
    /// # Arguments
    ///
    /// * `client_id` - The Azure AD application (client) ID
    /// * `tenant_id` - The Azure AD tenant ID or domain
    pub fn new(client_id: impl Into<String>, tenant_id: impl Into<String>) -> Self {
        Self {
            inner: Arc::new(BrowserFlowInner {
                client_id: client_id.into(),
                tenant_id: tenant_id.into(),
                redirect_port: None,
                http_client: reqwest::Client::new(),
            }),
        }
    }

    /// Set a specific port for the redirect URI.
    ///
    /// If not set, an available port will be chosen automatically.
    pub fn redirect_port(self, port: u16) -> Self {
        Self {
            inner: Arc::new(BrowserFlowInner {
                client_id: self.inner.client_id.clone(),
                tenant_id: self.inner.tenant_id.clone(),
                redirect_port: Some(port),
                http_client: reqwest::Client::new(),
            }),
        }
    }

    /// Start the browser authentication flow.
    ///
    /// This binds a local HTTP server and returns the auth URL.
    /// The consumer should open `auth_url` in a browser.
    pub async fn start(&self, resource: &str) -> Result<PendingBrowserAuth, AuthError> {
        // Bind listener
        let port = self.inner.redirect_port.unwrap_or(0);
        let addr = SocketAddr::from(([127, 0, 0, 1], port));
        let listener = TcpListener::bind(addr)
            .await
            .map_err(|e| AuthError::CallbackServerFailed(format!("Failed to bind: {}", e)))?;

        let local_addr = listener.local_addr().map_err(|e| {
            AuthError::CallbackServerFailed(format!("Failed to get local address: {}", e))
        })?;

        let redirect_uri = format!("http://localhost:{}/callback", local_addr.port());

        // Generate PKCE and state
        let code_verifier = generate_code_verifier();
        let code_challenge = generate_code_challenge(&code_verifier);
        let state = generate_state();

        // Build authorization URL
        let scope = scope_from_resource(resource);
        let auth_url = format!(
            "{}?client_id={}&response_type=code&redirect_uri={}&scope={}&state={}&code_challenge={}&code_challenge_method=S256",
            authorize_url_v2(&self.inner.tenant_id),
            urlencoding::encode(&self.inner.client_id),
            urlencoding::encode(&redirect_uri),
            urlencoding::encode(&scope),
            urlencoding::encode(&state),
            urlencoding::encode(&code_challenge),
        );

        Ok(PendingBrowserAuth {
            auth_url,
            redirect_uri,
            listener,
            state,
            code_verifier,
            flow: self.clone(),
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

impl std::fmt::Debug for BrowserFlow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BrowserFlow")
            .field("client_id", &self.inner.client_id)
            .field("tenant_id", &self.inner.tenant_id)
            .field("redirect_port", &self.inner.redirect_port)
            .finish()
    }
}

// =============================================================================
// PendingBrowserAuth
// =============================================================================

/// A pending browser authentication.
///
/// Open `auth_url` in a browser, then call `wait()` to complete authentication.
pub struct PendingBrowserAuth {
    /// URL to open in the browser
    pub auth_url: String,
    /// Local redirect URI (e.g., "http://localhost:12345/callback")
    pub redirect_uri: String,
    // Internal fields
    listener: TcpListener,
    state: String,
    code_verifier: String,
    flow: BrowserFlow,
    resource: String,
}

impl PendingBrowserAuth {
    /// Wait for the browser callback and exchange the code for a token.
    pub async fn wait(self) -> Result<AccessToken, AuthError> {
        let code = self.wait_for_callback().await?;
        self.exchange_code(&code).await
    }

    /// Wait for the browser callback with cancellation support.
    pub async fn wait_with_cancel(
        self,
        cancel: CancellationToken,
    ) -> Result<AccessToken, AuthError> {
        let code = tokio::select! {
            _ = cancel.cancelled() => {
                return Err(AuthError::BrowserCancelled);
            }
            result = self.wait_for_callback() => {
                result?
            }
        };

        // Check cancellation before exchange
        if cancel.is_cancelled() {
            return Err(AuthError::BrowserCancelled);
        }

        self.exchange_code(&code).await
    }

    async fn wait_for_callback(&self) -> Result<String, AuthError> {
        // Accept single connection
        let (stream, _) = self
            .listener
            .accept()
            .await
            .map_err(|e| AuthError::CallbackServerFailed(format!("Accept failed: {}", e)))?;

        let io = TokioIo::new(stream);

        // Channel to extract auth result from request handler
        let (tx, rx) = oneshot::channel::<Result<String, AuthError>>();
        let tx = Arc::new(std::sync::Mutex::new(Some(tx)));
        let expected_state = self.state.clone();

        let service = service_fn(move |req: Request<Incoming>| {
            let tx = tx.clone();
            let expected_state = expected_state.clone();
            async move {
                let result = handle_callback(req, &expected_state);

                // Determine response based on result (match by reference first)
                let (status, body) = match &result {
                    Ok(_) => (
                        hyper::StatusCode::OK,
                        "Authentication successful! You can close this window.",
                    ),
                    Err(AuthError::BrowserCancelled) => (
                        hyper::StatusCode::BAD_REQUEST,
                        "Authentication was cancelled or denied.",
                    ),
                    Err(_) => (
                        hyper::StatusCode::BAD_REQUEST,
                        "Authentication failed. Please try again.",
                    ),
                };

                // Send result through channel (only first request, moves result)
                if let Some(sender) = tx.lock().unwrap().take() {
                    let _ = sender.send(result);
                }

                let html = format!(
                    "<!DOCTYPE html><html><head><title>Authentication</title></head>\
                     <body><h1>{}</h1></body></html>",
                    body
                );

                Ok::<_, Infallible>(
                    Response::builder()
                        .status(status)
                        .header("Content-Type", "text/html")
                        .body(Full::new(Bytes::from(html)))
                        .unwrap(),
                )
            }
        });

        // Serve the single connection
        let conn = http1::Builder::new().serve_connection(io, service);

        // We need to drive the connection to completion
        // Connection errors are usually not critical (browser may close connection early)
        let _ = conn.await;

        // Get the result from the channel
        rx.await
            .map_err(|_| AuthError::CallbackServerFailed("No callback received".to_string()))?
    }

    async fn exchange_code(&self, code: &str) -> Result<AccessToken, AuthError> {
        let exchange = TokenExchange {
            http_client: &self.flow.inner.http_client,
            client_id: &self.flow.inner.client_id,
            tenant_id: &self.flow.inner.tenant_id,
            resource: &self.resource,
        };
        exchange
            .authorization_code(code, &self.redirect_uri, &self.code_verifier)
            .await
    }
}

impl std::fmt::Debug for PendingBrowserAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PendingBrowserAuth")
            .field("auth_url", &self.auth_url)
            .field("redirect_uri", &self.redirect_uri)
            .field("resource", &self.resource)
            .finish_non_exhaustive()
    }
}

// =============================================================================
// Callback Handler
// =============================================================================

/// Parse the OAuth callback and extract the authorization code
fn handle_callback(req: Request<Incoming>, expected_state: &str) -> Result<String, AuthError> {
    // Parse query string
    let query = req.uri().query().unwrap_or("");
    let params: HashMap<String, String> = url::form_urlencoded::parse(query.as_bytes())
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();

    // Check for error response
    if let Some(error) = params.get("error") {
        let description = params
            .get("error_description")
            .cloned()
            .unwrap_or_else(|| error.clone());

        return match error.as_str() {
            "access_denied" => Err(AuthError::BrowserCancelled),
            "consent_required" => Err(AuthError::BrowserCancelled),
            _ => Err(AuthError::CallbackServerFailed(description)),
        };
    }

    // Extract and validate state
    let state = params
        .get("state")
        .ok_or_else(|| AuthError::CallbackServerFailed("Missing state parameter".to_string()))?;

    if state != expected_state {
        return Err(AuthError::CallbackServerFailed(
            "State mismatch (possible CSRF attack)".to_string(),
        ));
    }

    // Extract authorization code
    let code = params
        .get("code")
        .ok_or_else(|| AuthError::CallbackServerFailed("Missing code parameter".to_string()))?;

    Ok(code.clone())
}

// =============================================================================
// AuthFlow Implementation
// =============================================================================

#[async_trait]
impl AuthFlow for BrowserFlow {
    /// Authenticate using browser flow.
    ///
    /// **Note:** This method will block waiting for the browser callback.
    /// For interactive applications, use `start()` directly so you can
    /// open the browser and display appropriate UI.
    async fn authenticate(&self, resource: &str) -> Result<AccessToken, AuthError> {
        let pending = self.start(resource).await?;
        // Note: Consumer should open pending.auth_url in browser
        // This just waits - won't work without browser interaction
        pending.wait().await
    }

    async fn refresh(&self, resource: &str, refresh_token: &str) -> Result<AccessToken, AuthError> {
        self.refresh(resource, refresh_token).await
    }
}
