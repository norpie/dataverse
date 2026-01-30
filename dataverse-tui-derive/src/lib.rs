//! Procedural macros for dataverse-tui.

mod settings;

use proc_macro::TokenStream;

/// Derives settings structure with automatic persistence.
///
/// Transforms fields into `Setting<T>` wrappers and generates loading logic.
///
/// # Example
///
/// ```rust,ignore
/// #[settings]
/// pub struct QueueSettings {
///     #[default = 5]
///     max_concurrency: usize,
/// }
/// ```
#[proc_macro_attribute]
pub fn settings(attr: TokenStream, item: TokenStream) -> TokenStream {
    settings::expand(attr, item)
}
