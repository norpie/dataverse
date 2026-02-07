//! Prelude module for convenient imports.
//!
//! ```ignore
//! use rafter::prelude::*;
//! ```

// Core traits and types
pub use crate::app::{App, AppConfig};
pub use crate::app_context::AppContext;
pub use crate::context_menu::{ContextMenuBuilder, ContextMenuDefinition};
pub use crate::global_context::GlobalContext;
pub use crate::handler_context::HandlerRegistry;
pub use crate::job::JobId;
pub use crate::keybinds::Keybinds;
pub use crate::modal::{Modal, ModalContext, ModalSize};
pub use crate::resource::{ProgressState, Resource, ResourceState};
pub use crate::runtime::{Runtime, RuntimeError};
pub use crate::state::State;
pub use crate::system::System;
pub use crate::toast::Toast;

// Derive macros
pub use rafter_derive::{
    app, app_impl, derived, handler, keybinds, modal, modal_impl, system, system_impl, watch,
};

// Re-export tuidom Element for page! macro
pub use tuidom::Element;

// Built-in widgets
pub use crate::widgets::*;
