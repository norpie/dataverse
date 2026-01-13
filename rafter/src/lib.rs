#![deny(clippy::panic)]

mod app;
mod app_context;
mod event;
mod global_context;
mod handler_context;
mod instance;
pub mod keybinds;
mod lifecycle;
mod modal;
pub mod prelude;
mod registration;
mod request;
mod resource;
pub mod runtime;
mod state;
mod system;
pub mod theme;
mod toast;
pub mod validation;
mod wakeup;
mod widget;
pub mod widgets;

pub use app::{App, AppConfig, BlurPolicy, PanicBehavior};
pub use app_context::{
    extract_panic_message, AppContext, AppError, AppErrorKind, AppModalRequest, ErrorReceiver,
    ErrorSender,
};
pub use event::{Event, FocusChanged, InstanceClosed, InstanceSpawned};
pub use handler_context::{EventData, Handler, HandlerContext, HandlerRegistry, WidgetHandlers};
pub use global_context::{
    ArcEvent, DataStore, GlobalContext, GlobalModalRequest, InstanceCommand, InstanceQuery,
    RequestTarget,
};
pub use instance::{
    AnyAppInstance, AppInstance, InstanceId, InstanceInfo, InstanceRegistry, RequestError,
    SpawnError,
};
pub use keybinds::{
    parse_key_string, HandlerId, KeyCombo, Keybind, KeybindClosures, KeybindEntry, KeybindError,
    KeybindInfo, KeybindScope, Keybinds, ParseKeyError,
};
pub use lifecycle::LifecycleHooks;
pub use modal::{Modal, ModalContext, ModalEntry, ModalKind, ModalPosition, ModalSize, SystemModal};
pub use registration::{
    registered_apps, registered_systems, AnySystem, AppRegistration, CloneableApp,
    SystemRegistration,
};
pub use request::Request;
pub use resource::{ProgressState, Resource, ResourceError, ResourceState};
pub use state::State;
pub use system::{Overlay, OverlayPosition, System};
pub use theme::{default_theme, RafterTheme};
pub use toast::{Toast, DEFAULT_TOAST_DURATION};
pub use wakeup::{channel as wakeup_channel, WakeupHandle, WakeupReceiver, WakeupSender};
pub use widget::{Widget, WidgetResult};

// Runtime
pub use runtime::{Runtime, RuntimeError};

// Re-export derive macros
pub use rafter_derive::{
    app, app_impl, event_handler, handler, keybinds, modal, modal_impl, page, request_handler,
    system, system_impl, theme, Event, Request,
};

// =============================================================================
// Page Macro Helpers
// =============================================================================

/// Trait for types that can be converted to page children.
/// Used by the page! macro to support both single elements and for-loop results.
pub trait IntoPageChildren {
    fn into_page_children(self) -> Vec<tuidom::Element>;
}

impl IntoPageChildren for tuidom::Element {
    fn into_page_children(self) -> Vec<tuidom::Element> {
        vec![self]
    }
}

impl IntoPageChildren for Vec<tuidom::Element> {
    fn into_page_children(self) -> Vec<tuidom::Element> {
        self
    }
}
