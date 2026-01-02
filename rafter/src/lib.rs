#![deny(clippy::panic)]

mod event;
mod keybinds;
mod request;
mod resource;
mod state;
mod toast;
pub mod validation;
mod wakeup;
mod widget;

pub use event::Event;
pub use keybinds::{
    parse_key_string, HandlerId, KeyCombo, Keybind, KeybindError, KeybindScope, Keybinds,
    ParseKeyError,
};
pub use request::Request;
pub use resource::{ProgressState, Resource, ResourceError, ResourceState};
pub use state::State;
pub use toast::{Toast, DEFAULT_TOAST_DURATION};
pub use wakeup::{channel as wakeup_channel, WakeupHandle, WakeupReceiver, WakeupSender};
pub use widget::{Widget, WidgetResult};

// Re-export derive macros
pub use rafter_derive::{event_handler, handler, keybinds, request_handler, Event, Request};
