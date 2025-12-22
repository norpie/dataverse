//! SelectItem trait for items that can be displayed in a Select widget.

/// Trait for items that can be displayed in a Select widget.
///
/// This trait provides the necessary information for rendering options
/// in a dropdown select.
///
/// # Example
///
/// ```ignore
/// struct Priority {
///     id: u32,
///     name: String,
/// }
///
/// impl SelectItem for Priority {
///     fn select_id(&self) -> String {
///         self.id.to_string()
///     }
///
///     fn select_label(&self) -> String {
///         self.name.clone()
///     }
/// }
/// ```
pub trait SelectItem {
    /// Unique identifier for this item.
    ///
    /// Used for tracking selection state.
    fn select_id(&self) -> String;

    /// Display text for this item.
    ///
    /// This is what gets shown in the dropdown and as the selected value.
    fn select_label(&self) -> String;
}

// Implement for String
impl SelectItem for String {
    fn select_id(&self) -> String {
        self.clone()
    }

    fn select_label(&self) -> String {
        self.clone()
    }
}

// Implement for &str
impl SelectItem for &str {
    fn select_id(&self) -> String {
        (*self).to_string()
    }

    fn select_label(&self) -> String {
        (*self).to_string()
    }
}

// Implement for (id, label) tuples
impl<S1, S2> SelectItem for (S1, S2)
where
    S1: AsRef<str>,
    S2: AsRef<str>,
{
    fn select_id(&self) -> String {
        self.0.as_ref().to_string()
    }

    fn select_label(&self) -> String {
        self.1.as_ref().to_string()
    }
}
