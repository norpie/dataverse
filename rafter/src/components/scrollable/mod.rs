//! Scrollable component - a container with scrolling support.

pub mod events;
pub mod render;
mod state;

// Re-export scrollbar types from the shared module for backwards compatibility
pub use super::scrollbar::{ScrollbarConfig, ScrollbarDrag, ScrollbarGeometry, ScrollbarVisibility};
pub use state::{Scrollable, ScrollableId, ScrollDirection};
