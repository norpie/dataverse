//! Scrollable component - a container with scrolling support.

pub mod events;
pub mod render;
mod state;

pub use events::ScrollbarDrag;
pub use state::{
    Scrollable, ScrollableId, ScrollDirection, ScrollbarConfig, ScrollbarGeometry,
    ScrollbarVisibility,
};
