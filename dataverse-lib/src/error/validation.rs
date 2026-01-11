//! Validation error types

/// Error information for a specific field that failed validation.
#[derive(Debug, Clone)]
pub struct FieldValidationError {
    /// The field that failed validation.
    pub field: String,
    /// Human-readable validation error message.
    pub message: String,
    /// Optional error code.
    pub code: Option<String>,
}

impl FieldValidationError {
    /// Creates a new field validation error.
    pub fn new(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
            code: None,
        }
    }

    /// Creates a new field validation error with an error code.
    pub fn with_code(field: impl Into<String>, message: impl Into<String>, code: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
            code: Some(code.into()),
        }
    }
}

impl std::fmt::Display for FieldValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(code) = &self.code {
            write!(f, "{}: {} ({})", self.field, self.message, code)
        } else {
            write!(f, "{}: {}", self.field, self.message)
        }
    }
}
