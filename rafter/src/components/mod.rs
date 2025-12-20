//! UI components with self-managed state.
//!
//! Each component lives in its own module with:
//! - `state.rs` - the component state type
//! - `render.rs` - rendering logic
//! - `mod.rs` - public exports

pub mod input;
pub mod scrollable;

pub use input::{Input, InputId};
pub use scrollable::{Scrollable, ScrollableId, ScrollDirection, ScrollbarConfig, ScrollbarVisibility};
