//! AutocompleteItem trait for items that can be displayed in an Autocomplete widget.

/// Trait for items that can be displayed in an Autocomplete widget.
///
/// This trait provides the necessary information for rendering options
/// in a filtered dropdown.
///
/// # Example
///
/// ```ignore
/// struct Country {
///     code: String,
///     name: String,
/// }
///
/// impl AutocompleteItem for Country {
///     fn autocomplete_id(&self) -> String {
///         self.code.clone()
///     }
///
///     fn autocomplete_label(&self) -> String {
///         self.name.clone()
///     }
/// }
/// ```
pub trait AutocompleteItem {
    /// Unique identifier for this item.
    ///
    /// Used for tracking selection state.
    fn autocomplete_id(&self) -> String;

    /// Display text for this item.
    ///
    /// This is what gets shown in the dropdown and used for filtering.
    fn autocomplete_label(&self) -> String;
}

// Implement for String
impl AutocompleteItem for String {
    fn autocomplete_id(&self) -> String {
        self.clone()
    }

    fn autocomplete_label(&self) -> String {
        self.clone()
    }
}

// Implement for &str
impl AutocompleteItem for &str {
    fn autocomplete_id(&self) -> String {
        (*self).to_string()
    }

    fn autocomplete_label(&self) -> String {
        (*self).to_string()
    }
}

// Implement for (id, label) tuples
impl<S1, S2> AutocompleteItem for (S1, S2)
where
    S1: AsRef<str>,
    S2: AsRef<str>,
{
    fn autocomplete_id(&self) -> String {
        self.0.as_ref().to_string()
    }

    fn autocomplete_label(&self) -> String {
        self.1.as_ref().to_string()
    }
}
