//! Shared scrollbar infrastructure for scrollable components.
//!
//! This module provides:
//! - `ScrollbarState` trait for components that support scrolling
//! - Configuration types (`ScrollbarConfig`, `ScrollbarVisibility`)
//! - Geometry types for hit testing (`ScrollbarGeometry`)
//! - Rendering functions for vertical/horizontal scrollbars
//! - Event handling helpers for scrollbar interactions
//! - Drag state management

mod events;
mod render;
mod state;
mod types;

pub use events::{
    handle_scroll, handle_scrollbar_click, handle_scrollbar_drag, handle_scrollbar_release,
};
pub use render::{render_horizontal_scrollbar, render_vertical_scrollbar};
pub use state::ScrollbarState;
pub use types::{ScrollbarConfig, ScrollbarDrag, ScrollbarGeometry, ScrollbarVisibility};
