//! Validation result types.

use crate::context::AppContext;

/// Information about a single field validation error.
#[derive(Debug, Clone)]
pub struct FieldError {
    /// Field name (from `.field()` call).
    pub field_name: String,
    /// Widget ID (for focusing).
    pub widget_id: String,
    /// Error message.
    pub message: String,
}

/// Result of validating one or more fields.
#[derive(Debug, Clone)]
#[derive(Default)]
pub enum ValidationResult {
    /// All fields passed validation.
    #[default]
    Valid,
    /// One or more fields failed validation.
    Invalid(Vec<FieldError>),
}

impl ValidationResult {
    /// Check if all fields passed validation.
    pub fn is_valid(&self) -> bool {
        matches!(self, Self::Valid)
    }

    /// Check if any field failed validation.
    pub fn is_invalid(&self) -> bool {
        !self.is_valid()
    }

    /// Get all validation errors.
    pub fn errors(&self) -> &[FieldError] {
        match self {
            Self::Valid => &[],
            Self::Invalid(errors) => errors,
        }
    }

    /// Get the first validation error (if any).
    pub fn first_error(&self) -> Option<&FieldError> {
        self.errors().first()
    }

    /// Focus the first invalid field.
    pub fn focus_first(&self, cx: &AppContext) {
        if let Some(err) = self.first_error() {
            cx.focus(err.widget_id.clone());
        }
    }

    /// Show a toast with the first error or error count.
    pub fn toast_errors(&self, cx: &AppContext) {
        match self.errors().len() {
            0 => {}
            1 => cx.toast_error(&self.errors()[0].message),
            n => cx.toast_error(format!("{} validation errors", n)),
        }
    }

    /// Show a toast for each validation error.
    pub fn toast_all_errors(&self, cx: &AppContext) {
        for err in self.errors() {
            cx.toast_error(&err.message);
        }
    }
}

