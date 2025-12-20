//! Shared scrollbar infrastructure for scrollable components.
//!
//! This module provides:
//! - `ScrollbarState` trait for components that support scrolling
//! - Configuration types (`ScrollbarConfig`, `ScrollbarVisibility`)
//! - Geometry types for hit testing (`ScrollbarGeometry`)
//! - Rendering functions for vertical/horizontal scrollbars
//! - Drag state management

mod render;
mod state;
mod types;

pub use render::{render_horizontal_scrollbar, render_vertical_scrollbar};
pub use state::ScrollbarState;
pub use types::{ScrollbarConfig, ScrollbarDrag, ScrollbarGeometry, ScrollbarVisibility};
