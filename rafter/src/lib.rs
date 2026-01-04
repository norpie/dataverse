#![deny(clippy::panic)]

mod app;
mod app_context;
mod event;
mod global_context;
mod keybinds;
mod modal;
mod registration;
mod request;
mod resource;
mod state;
mod system;
mod toast;
pub mod validation;
mod wakeup;
mod widget;

pub use app::{App, AppConfig, PanicBehavior};
pub use app_context::AppContext;
pub use global_context::GlobalContext;
pub use event::Event;
pub use keybinds::{
    parse_key_string, HandlerId, KeyCombo, Keybind, KeybindError, KeybindScope, Keybinds,
    ParseKeyError,
};
pub use modal::{Modal, ModalContext, ModalPosition, ModalSize};
pub use registration::{
    registered_apps, registered_systems, AnySystem, AppRegistration, CloneableApp,
    SystemRegistration,
};
pub use request::Request;
pub use resource::{ProgressState, Resource, ResourceError, ResourceState};
pub use state::State;
pub use system::{Overlay, OverlayPosition, System};
pub use toast::{Toast, DEFAULT_TOAST_DURATION};
pub use wakeup::{channel as wakeup_channel, WakeupHandle, WakeupReceiver, WakeupSender};
pub use widget::{Widget, WidgetResult};

// Re-export derive macros
pub use rafter_derive::{
    app, app_impl, event_handler, handler, keybinds, modal, modal_impl, request_handler, system,
    system_impl, theme, Event, Request,
};
