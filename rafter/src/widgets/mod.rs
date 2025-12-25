//! UI widgets with self-managed state.
//!
//! Each widget lives in its own module with:
//! - `state.rs` - the widget state type
//! - `render.rs` - rendering logic
//! - `events.rs` - event handling implementation
//! - `mod.rs` - public exports

pub mod autocomplete;
pub mod button;
pub mod checkbox;
pub mod collapsible;
pub mod events;
pub mod input;
pub mod list;
pub mod radio;
pub mod scroll_area;
pub mod scrollbar;
pub mod select;
pub mod selection;
pub mod table;
mod traits;
pub mod tree;

pub use autocomplete::{fuzzy_filter, AutocompleteItem, FilterMatch};
pub use button::Button;
pub use checkbox::{Checkbox, CheckboxId};
pub use collapsible::{Collapsible, CollapsibleId};
pub use events::{EventResult, WidgetEvents};
pub use input::{Input, InputId};
pub use list::{AnyList, List, ListId, ListItem, Selection, SelectionMode};
pub use radio::{RadioGroup, RadioGroupId};
pub use scroll_area::{ScrollArea, ScrollAreaId, ScrollDirection};
pub use scrollbar::{
    ScrollbarConfig, ScrollbarDrag, ScrollbarGeometry, ScrollbarState, ScrollbarVisibility,
};
pub use select::{Select, SelectId, SelectItem};
pub use table::{Alignment, AnyTable, Column, Table, TableId, TableRow};
pub use traits::{
    // Legacy traits (still used by existing widgets, will be migrated in Phase 5)
    AnySelectable,
    // New unified widget system
    AnyWidget,
    RenderContext,
    Scrollable,
    ScrollableWidget,
    Selectable,
    SelectableWidget,
    WidgetHandlers,
};
pub use tree::{AnyTree, FlatNode, Tree, TreeId, TreeItem};
