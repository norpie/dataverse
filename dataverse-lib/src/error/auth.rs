//! Authentication error types

/// Errors that can occur during authentication flows.
#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    /// Device code flow expired before user completed authentication.
    #[error("Device code expired")]
    DeviceCodeExpired,

    /// User declined the device code authentication request.
    #[error("Device code declined by user")]
    DeviceCodeDeclined,

    /// Invalid username or password.
    #[error("Invalid credentials")]
    InvalidCredentials,

    /// Access token expired and refresh failed.
    #[error("Token expired and refresh failed: {message}")]
    TokenExpired { message: String },

    /// The specified tenant ID is invalid or not found.
    #[error("Invalid tenant: {tenant}")]
    InvalidTenant { tenant: String },

    /// The specified client ID is invalid or not authorized.
    #[error("Invalid client: {client_id}")]
    InvalidClient { client_id: String },

    /// Network error during authentication.
    #[error("Network error during auth: {0}")]
    Network(#[from] reqwest::Error),

    /// Failed to parse authentication response.
    #[error("Auth response parse error: {0}")]
    Parse(String),

    /// Browser authentication was cancelled.
    #[error("Browser authentication cancelled")]
    BrowserCancelled,

    /// Failed to start local callback server for browser auth.
    #[error("Failed to start callback server: {0}")]
    CallbackServerFailed(String),
}
