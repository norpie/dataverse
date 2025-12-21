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
pub mod scroll_area;
pub mod scrollbar;
pub mod selection;
pub mod table;
mod traits;
pub mod tree;

pub use events::{ComponentEvents, EventResult};
pub use input::{Input, InputId};
pub use list::{AnyList, List, ListId, ListItem, Selection, SelectionMode};
pub use scroll_area::{ScrollArea, ScrollAreaId, ScrollDirection};
pub use scrollbar::{
    ScrollbarConfig, ScrollbarDrag, ScrollbarGeometry, ScrollbarState, ScrollbarVisibility,
};
pub use table::{Alignment, AnyTable, Column, Table, TableId, TableRow};
pub use traits::ScrollableComponent;
pub use tree::{AnyTree, FlatNode, Tree, TreeId, TreeItem};
