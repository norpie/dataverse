//! Autocomplete widget - text input with fuzzy-filtered dropdown suggestions.

mod filter;
mod item;

pub use filter::{fuzzy_filter, FilterMatch};
pub use item::AutocompleteItem;
