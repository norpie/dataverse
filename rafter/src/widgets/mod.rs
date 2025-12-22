//! UI widgets with self-managed state.
//!
//! Each widget lives in its own module with:
//! - `state.rs` - the widget state type
//! - `render.rs` - rendering logic
//! - `events.rs` - event handling implementation
//! - `mod.rs` - public exports

pub mod button;
pub mod checkbox;
pub mod events;
pub mod input;
pub mod list;
pub mod radio;
pub mod scroll_area;
pub mod scrollbar;
pub mod selection;
pub mod table;
mod traits;
pub mod tree;

pub use button::Button;
pub use checkbox::{Checkbox, CheckboxId};
pub use radio::{RadioGroup, RadioGroupId};
pub use events::{WidgetEvents, EventResult};
pub use input::{Input, InputId};
pub use list::{AnyList, List, ListId, ListItem, Selection, SelectionMode};
pub use scroll_area::{ScrollArea, ScrollAreaId, ScrollDirection};
pub use scrollbar::{
    ScrollbarConfig, ScrollbarDrag, ScrollbarGeometry, ScrollbarState, ScrollbarVisibility,
};
pub use table::{Alignment, AnyTable, Column, Table, TableId, TableRow};
pub use traits::{
    // Legacy traits (still used by existing widgets, will be migrated in Phase 5)
    AnySelectable, ScrollableWidget, SelectableWidget,
    // New unified widget system
    AnyWidget, RenderContext, Scrollable, Selectable, WidgetHandlers,
};
pub use tree::{AnyTree, FlatNode, Tree, TreeId, TreeItem};
