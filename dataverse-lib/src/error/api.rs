//! API error types

use std::time::Duration;

use super::DataverseErrorDetail;

/// Errors that can occur during API calls.
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    /// HTTP error response from the API.
    #[error("HTTP {status}: {message}")]
    Http {
        /// HTTP status code.
        status: u16,
        /// Error message.
        message: String,
        /// Dataverse error code, if available.
        code: Option<String>,
        /// Detailed error information from Dataverse.
        inner: Option<Box<DataverseErrorDetail>>,
    },

    /// Network error during API call.
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    /// Request timed out.
    #[error("Timeout after {0:?}")]
    Timeout(Duration),

    /// Invalid URL provided.
    #[error("Invalid URL: {0}")]
    InvalidUrl(String),

    /// Failed to parse API response.
    #[error("Response parse error: {message}")]
    Parse {
        /// Description of the parse error.
        message: String,
        /// Raw response body, if available.
        body: Option<String>,
    },
}

impl ApiError {
    /// Creates a new HTTP error.
    pub fn http(status: u16, message: impl Into<String>) -> Self {
        Self::Http {
            status,
            message: message.into(),
            code: None,
            inner: None,
        }
    }

    /// Creates a new HTTP error with Dataverse error details.
    pub fn http_with_detail(status: u16, message: impl Into<String>, detail: DataverseErrorDetail) -> Self {
        Self::Http {
            status,
            message: message.into(),
            code: Some(detail.code.clone()),
            inner: Some(Box::new(detail)),
        }
    }

    /// Creates a new parse error.
    pub fn parse(message: impl Into<String>) -> Self {
        Self::Parse {
            message: message.into(),
            body: None,
        }
    }

    /// Creates a new parse error with the raw response body.
    pub fn parse_with_body(message: impl Into<String>, body: impl Into<String>) -> Self {
        Self::Parse {
            message: message.into(),
            body: Some(body.into()),
        }
    }

    /// Returns the HTTP status code if this is an HTTP error.
    pub fn status_code(&self) -> Option<u16> {
        match self {
            Self::Http { status, .. } => Some(*status),
            _ => None,
        }
    }

    /// Returns the Dataverse error code if available.
    pub fn error_code(&self) -> Option<&str> {
        match self {
            Self::Http { code, .. } => code.as_deref(),
            _ => None,
        }
    }

    /// Returns the Dataverse error detail if available.
    pub fn dataverse_detail(&self) -> Option<&DataverseErrorDetail> {
        match self {
            Self::Http { inner, .. } => inner.as_deref(),
            _ => None,
        }
    }

    /// Returns `true` if this error is potentially retryable.
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Http { status, .. } => matches!(status, 429 | 500 | 502 | 503 | 504),
            Self::Network(_) => true,
            Self::Timeout(_) => true,
            _ => false,
        }
    }
}
