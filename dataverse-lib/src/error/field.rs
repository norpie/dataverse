//! FieldError for Record accessors

/// Error type for field access operations on Record.
#[derive(Debug, Clone, thiserror::Error)]
pub enum FieldError {
    /// The requested field does not exist in the record.
    #[error("Field '{field}' not found in record")]
    Missing { field: String },

    /// The field exists but has a different type than requested.
    #[error("Field '{field}' type mismatch: expected {expected}, got {actual}")]
    TypeMismatch {
        field: String,
        expected: &'static str,
        actual: &'static str,
    },
}

impl FieldError {
    /// Creates a new missing field error.
    pub fn missing(field: impl Into<String>) -> Self {
        Self::Missing {
            field: field.into(),
        }
    }

    /// Creates a new type mismatch error.
    pub fn type_mismatch(field: impl Into<String>, expected: &'static str, actual: &'static str) -> Self {
        Self::TypeMismatch {
            field: field.into(),
            expected,
            actual,
        }
    }
}
