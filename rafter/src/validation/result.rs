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
#[derive(Debug, Clone, Default)]
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

    /// Get the widget ID of the first invalid field (for focusing).
    pub fn first_invalid_widget(&self) -> Option<&str> {
        self.first_error().map(|e| e.widget_id.as_str())
    }

    // TODO: Add focus_first(&self, cx: &AppContext) when AppContext exists
    // TODO: Add toast_errors(&self, cx: &AppContext) when AppContext exists
    // TODO: Add toast_all_errors(&self, cx: &AppContext) when AppContext exists
}
