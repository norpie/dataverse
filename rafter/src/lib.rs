// Core modules
pub mod app;
pub mod context;
pub mod event;
pub mod node;
pub mod request;
pub mod resource;
pub mod runtime;
pub mod state;

// Grouped modules
pub mod input;
pub mod layers;
pub mod styling;

// Supporting modules
pub mod utils;
pub mod validation;
pub mod widgets;

pub use rafter_derive::*;
pub use runtime::Runtime;

// Re-exports for macro-generated code (keybinds!, page!, etc.)
// These allow paths like `rafter::keybinds::*` to work in generated code
pub use input::events;
pub use input::keybinds;
pub use layers::modal;
pub use styling::color;
pub use styling::style;
pub use styling::theme;

pub mod prelude {
    // App
    pub use crate::app::App;

    // Styling
    pub use crate::styling::{Color, Style, StyleColor};
    pub use crate::styling::{DefaultTheme, Theme};

    // Input
    pub use crate::input::{ClickEvent, ClickKind, Modifiers};
    pub use crate::input::{FocusState, KeybindError, KeybindInfo, Keybinds};

    // Layers
    pub use crate::layers::{Modal, ModalContext, ModalPosition, ModalSize};

    // Widgets
    pub use crate::widgets::{Alignment, Column, Table, TableId, TableRow};
    pub use crate::widgets::{AnyWidget, Scrollable, Selectable, WidgetHandlers};
    pub use crate::widgets::{
        Button, Checkbox, Collapsible, Input, RadioGroup, ScrollArea, ScrollDirection,
        ScrollbarConfig, ScrollbarVisibility,
    };
    pub use crate::widgets::{List, ListId, ListItem, Selection, SelectionMode};
    pub use crate::widgets::{Select, SelectId, SelectItem};
    pub use crate::widgets::{Tree, TreeId, TreeItem};

    // Core
    pub use crate::context::{AppContext, Toast, ToastLevel};
    pub use crate::event::Event;
    pub use crate::node::Node;
    pub use crate::request::{Request, RequestError};
    pub use crate::resource::{ProgressState, Resource, ResourceState};
    pub use crate::runtime::Runtime;
    pub use crate::state::State;

    // Validation
    pub use crate::validation::{
        ErrorDisplay, FieldError, Validatable, ValidationResult, Validator,
    };

    pub use rafter_derive::*;
}
