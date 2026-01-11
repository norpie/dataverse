//! Dataverse-specific error types

use std::collections::HashMap;

/// Detailed error information from Dataverse API responses.
///
/// Dataverse returns structured error information that can include
/// nested inner errors and additional metadata.
#[derive(Debug, Clone)]
pub struct DataverseErrorDetail {
    /// The error code (e.g., "0x80040217").
    pub code: String,
    /// Human-readable error message.
    pub message: String,
    /// Nested inner error, if any.
    pub inner_error: Option<Box<DataverseErrorDetail>>,
    /// Additional error metadata.
    pub additional_info: HashMap<String, serde_json::Value>,
}

impl DataverseErrorDetail {
    /// Creates a new error detail with the given code and message.
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            inner_error: None,
            additional_info: HashMap::new(),
        }
    }

    /// Returns the innermost error in the chain.
    pub fn innermost(&self) -> &DataverseErrorDetail {
        let mut current = self;
        while let Some(inner) = &current.inner_error {
            current = inner;
        }
        current
    }

    /// Checks if this error or any inner error has the given code.
    pub fn has_code(&self, code: &str) -> bool {
        if self.code == code {
            return true;
        }
        if let Some(inner) = &self.inner_error {
            return inner.has_code(code);
        }
        false
    }
}

impl std::fmt::Display for DataverseErrorDetail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)
    }
}
