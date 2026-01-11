//! Error types

mod api;
mod auth;
mod concurrency;
mod dataverse;
mod field;
mod validation;

pub use api::*;
pub use auth::*;
pub use dataverse::*;
pub use field::*;
pub use validation::*;

use std::time::Duration;
use uuid::Uuid;

/// Errors that can occur during metadata operations.
#[derive(Debug, thiserror::Error)]
pub enum MetadataError {
    /// The specified entity was not found.
    #[error("Entity '{name}' not found")]
    EntityNotFound { name: String },

    /// The specified attribute was not found on the entity.
    #[error("Attribute '{attribute}' not found on entity '{entity}'")]
    AttributeNotFound { entity: String, attribute: String },

    /// The specified relationship was not found.
    #[error("Relationship '{name}' not found")]
    RelationshipNotFound { name: String },

    /// The specified global option set was not found.
    #[error("Global option set '{name}' not found")]
    OptionSetNotFound { name: String },

    /// Failed to resolve an entity logical name to its entity set name.
    #[error("Failed to resolve entity '{logical_name}' to entity set name")]
    ResolutionFailed {
        logical_name: String,
        #[source]
        source: Box<Error>,
    },

    /// Failed to fetch metadata from the API.
    #[error("Failed to fetch metadata: {0}")]
    FetchFailed(#[from] ApiError),
}

/// Top-level error type for the Dataverse client library.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Authentication error.
    #[error("Authentication failed: {0}")]
    Auth(#[from] AuthError),

    /// API error.
    #[error("API error: {0}")]
    Api(#[from] ApiError),

    /// Metadata error.
    #[error("Metadata error: {0}")]
    Metadata(#[from] MetadataError),

    /// Record not found.
    #[error("Record not found: {entity} with id {id}")]
    NotFound { entity: String, id: Uuid },

    /// Permission denied.
    #[error("Permission denied: {message}")]
    Permission {
        message: String,
        code: Option<String>,
    },

    /// Concurrency conflict - record was modified by another user.
    #[error("Concurrency conflict: record was modified")]
    Concurrency { current_etag: Option<String> },

    /// Rate limited by the server.
    #[error("Rate limited, retry after {retry_after:?}")]
    RateLimit { retry_after: Option<Duration> },

    /// Validation error.
    #[error("Validation error: {message}")]
    Validation {
        message: String,
        errors: Vec<FieldValidationError>,
    },

    /// Field access error.
    #[error("Field error: {0}")]
    Field(#[from] FieldError),

    /// Serialization/deserialization error.
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Invalid operation.
    #[error("Invalid operation: {0}")]
    InvalidOperation(String),

    /// Batch size exceeded the maximum allowed.
    #[error("Batch size exceeded: {count} operations (max {max})")]
    BatchSizeExceeded { count: usize, max: usize },
}

impl Error {
    /// Returns `true` if this error is potentially retryable.
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Api(e) => e.is_retryable(),
            Self::RateLimit { .. } => true,
            Self::Concurrency { .. } => true,
            _ => false,
        }
    }

    /// Returns the HTTP status code if this is an API error.
    pub fn status_code(&self) -> Option<u16> {
        match self {
            Self::Api(e) => e.status_code(),
            Self::NotFound { .. } => Some(404),
            Self::Permission { .. } => Some(403),
            Self::RateLimit { .. } => Some(429),
            _ => None,
        }
    }

    /// Returns the Dataverse error code if available.
    pub fn error_code(&self) -> Option<&str> {
        match self {
            Self::Api(e) => e.error_code(),
            Self::Permission { code, .. } => code.as_deref(),
            _ => None,
        }
    }

    /// Returns the retry-after duration if this is a rate limit error.
    pub fn retry_after(&self) -> Option<Duration> {
        match self {
            Self::RateLimit { retry_after } => *retry_after,
            _ => None,
        }
    }

    /// Returns the Dataverse error detail if available.
    pub fn dataverse_detail(&self) -> Option<&DataverseErrorDetail> {
        match self {
            Self::Api(e) => e.dataverse_detail(),
            _ => None,
        }
    }

    /// Checks if this error has the specified Dataverse error code.
    pub fn is_error_code(&self, code: &str) -> bool {
        self.error_code().is_some_and(|c| c == code)
    }
}

/// Common Dataverse error codes.
pub mod error_codes {
    /// Object does not exist.
    pub const OBJECT_DOES_NOT_EXIST: &str = "0x80040217";
    /// Privilege denied.
    pub const PRIVILEGE_DENIED: &str = "0x80040220";
    /// Cannot delete due to association.
    pub const CANNOT_DELETE_DUE_TO_ASSOCIATION: &str = "0x80040227";
    /// Duplicate detected.
    pub const DUPLICATE_DETECTED: &str = "0x80040333";
    /// Concurrency version mismatch.
    pub const CONCURRENCY_VERSION_MISMATCH: &str = "0x80060882";
    /// SQL timeout.
    pub const SQL_TIMEOUT: &str = "0x80044151";
    /// Service unavailable.
    pub const SERVICE_UNAVAILABLE: &str = "0x8005F103";
    /// Throttling.
    pub const THROTTLING: &str = "0x8005F102";
    /// Invalid argument.
    pub const INVALID_ARGUMENT: &str = "0x80040203";
    /// Attribute does not exist.
    pub const ATTRIBUTE_DOES_NOT_EXIST: &str = "0x80047019";
    /// Entity does not exist.
    pub const ENTITY_DOES_NOT_EXIST: &str = "0x80060888";
}

/// Result type alias for operations that can fail with [`Error`].
pub type Result<T> = std::result::Result<T, Error>;
