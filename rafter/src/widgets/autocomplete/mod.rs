//! Autocomplete widget - text input with fuzzy-filtered dropdown suggestions.

mod events;
mod filter;
mod item;
mod render;
mod state;

pub use filter::{FilterMatch, fuzzy_filter};
pub use item::AutocompleteItem;
pub use state::{Autocomplete, AutocompleteId};
