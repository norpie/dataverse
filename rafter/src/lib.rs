pub mod app;
pub mod color;
pub mod context;
pub mod events;
pub mod keybinds;
pub mod node;
pub mod resource;
pub mod runtime;
pub mod state;
pub mod style;

pub use rafter_derive::*;

pub mod prelude {
    pub use crate::app::App;
    pub use crate::color::Color;
    pub use crate::context::AppContext;
    pub use crate::events::{ClickEvent, ClickKind, Modifiers};
    pub use crate::keybinds::Keybinds;
    pub use crate::node::Node;
    pub use crate::resource::{ProgressState, Resource};
    pub use crate::runtime::Runtime;
    pub use crate::state::State;
    pub use crate::style::Style;

    pub use rafter_derive::*;
}
