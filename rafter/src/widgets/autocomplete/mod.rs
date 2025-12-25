//! Autocomplete widget - text input with fuzzy-filtered dropdown suggestions.

mod events;
mod filter;
mod item;
mod render;
mod state;

pub use filter::{fuzzy_filter, FilterMatch};
pub use item::AutocompleteItem;
pub use state::{Autocomplete, AutocompleteId};
