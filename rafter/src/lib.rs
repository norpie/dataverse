pub mod app;
pub mod color;
pub mod components;
pub mod context;
pub mod events;
pub mod focus;
pub mod keybinds;
pub mod modal;
pub mod node;
pub mod resource;
pub mod runtime;
pub mod state;
pub mod style;
pub mod theme;
pub mod utils;

pub use rafter_derive::*;
pub use runtime::Runtime;

pub mod prelude {
    pub use crate::app::App;
    pub use crate::color::{Color, StyleColor};
    pub use crate::components::{Alignment, Column, Table, TableId, TableRow};
    pub use crate::components::{
        Input, ScrollArea, ScrollDirection, ScrollbarConfig, ScrollbarVisibility,
    };
    pub use crate::components::{List, ListId, ListItem, Selection, SelectionMode};
    pub use crate::components::{Tree, TreeId, TreeItem};
    pub use crate::context::{AppContext, Toast, ToastLevel};
    pub use crate::events::{ClickEvent, ClickKind, Modifiers};
    pub use crate::focus::FocusState;
    pub use crate::keybinds::{KeybindError, KeybindInfo, Keybinds};
    pub use crate::modal::{Modal, ModalContext, ModalPosition, ModalSize};
    pub use crate::node::Node;
    pub use crate::resource::{ProgressState, Resource, ResourceState};
    pub use crate::runtime::Runtime;
    pub use crate::state::State;
    pub use crate::style::Style;
    pub use crate::theme::{DefaultTheme, Theme};

    pub use rafter_derive::*;
}
