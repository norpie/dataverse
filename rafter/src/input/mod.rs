//! Input module - keybinds, focus management, and input events.

pub mod events;
pub mod focus;
pub mod keybinds;

pub use events::{
    ClickEvent, ClickKind, InputEvent, Modifiers, Position, ScrollDirection, ScrollEvent,
    SubmitEvent,
};
pub use focus::FocusState;
pub use keybinds::{
    HandlerId, Key, KeyCombo, Keybind, KeybindError, KeybindInfo, KeybindScope, Keybinds,
};
