//! UI components with self-managed state.
//!
//! Each component lives in its own module with:
//! - `state.rs` - the component state type
//! - `render.rs` - rendering logic
//! - `events.rs` - event handling implementation
//! - `mod.rs` - public exports

pub mod events;
pub mod input;
pub mod list;
pub mod scrollable;

pub use events::{ComponentEvents, EventResult};
pub use input::{Input, InputId};
pub use list::{
    ActivateEvent, CursorMoveEvent, List, ListId, ListItem, Selection, SelectionChangeEvent,
    SelectionMode,
};
pub use scrollable::{
    Scrollable, ScrollDirection, ScrollableId, ScrollbarConfig, ScrollbarVisibility,
};
