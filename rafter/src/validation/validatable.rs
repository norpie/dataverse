use super::ErrorDisplay;

/// Trait for widgets that can be validated.
///
/// Widgets implement this trait to integrate with the validation system.
pub trait Validatable: Send + Sync {
    /// The value type used for validation.
    type Value;

    /// Extract the current value for validation.
    fn validation_value(&self) -> Self::Value;

    /// Set a validation error on this widget.
    fn set_error(&self, msg: impl Into<String>);

    /// Clear the validation error.
    fn clear_error(&self);

    /// Check if the widget has a validation error.
    fn has_error(&self) -> bool;

    /// Get the current validation error message (if any).
    fn error(&self) -> Option<String>;

    /// Get the widget ID for focusing.
    fn widget_id(&self) -> String;

    /// Get the error display mode.
    fn error_display(&self) -> ErrorDisplay;

    /// Set the error display mode.
    fn set_error_display(&self, display: ErrorDisplay);
}
