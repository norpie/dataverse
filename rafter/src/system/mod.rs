//! System handlers for global keybinds and event handling.
//!
//! Systems are like invisible apps - they have keybinds and handlers but no view.
//! System keybinds are checked before app keybinds (highest priority).
//!
//! # Example
//!
//! ```ignore
//! #[system]
//! struct GlobalKeys;
//!
//! #[system_impl]
//! impl GlobalKeys {
//!     #[keybinds]
//!     fn keybinds() -> Keybinds {
//!         keybinds! {
//!             "Ctrl+q" => quit,
//!             "Ctrl+Tab" => next_app,
//!         }
//!     }
//!
//!     #[handler]
//!     fn quit(cx: &AppContext) {
//!         cx.exit();
//!     }
//!
//!     #[handler]
//!     async fn next_app(cx: &AppContext) {
//!         // Cycle to next app instance
//!     }
//! }
//! ```

use std::any::{Any, TypeId};
use std::future::Future;
use std::pin::Pin;

use crate::context::AppContext;
use crate::input::keybinds::{HandlerId, Keybinds};

/// Trait that all systems must implement.
///
/// This is typically implemented via the `#[system]` and `#[system_impl]` macros.
///
/// Systems are similar to apps but:
/// - Have no view/page
/// - Have no lifecycle hooks
/// - Keybinds are checked before app keybinds
/// - Are automatically instantiated at runtime startup
pub trait System: Clone + Send + Sync + 'static {
    /// Get the system's display name.
    fn name(&self) -> &'static str;

    /// Get the system's keybinds.
    fn keybinds(&self) -> Keybinds {
        Keybinds::new()
    }

    /// Dispatch a handler by ID.
    fn dispatch(&self, handler_id: &HandlerId, cx: &AppContext) {
        log::warn!(
            "Default dispatch called for handler '{}' - macro may not have generated dispatch",
            handler_id.0
        );
        let _ = cx;
    }

    /// Check if this system has a handler for the given event type.
    fn has_event_handler(&self, event_type: TypeId) -> bool {
        let _ = event_type;
        false
    }

    /// Check if this system has a handler for the given request type.
    fn has_request_handler(&self, request_type: TypeId) -> bool {
        let _ = request_type;
        false
    }

    /// Dispatch an event to this system's event handlers.
    ///
    /// Returns true if the event was handled (a handler exists for this event type).
    fn dispatch_event(
        &self,
        event_type: TypeId,
        event: Box<dyn Any + Send + Sync>,
        cx: &AppContext,
    ) -> bool {
        let _ = (event_type, event, cx);
        false
    }

    /// Dispatch a request to this system's request handlers.
    ///
    /// Returns Some(future) if a handler exists for this request type.
    fn dispatch_request(
        &self,
        request_type: TypeId,
        request: Box<dyn Any + Send + Sync>,
        cx: &AppContext,
    ) -> Option<Pin<Box<dyn Future<Output = Box<dyn Any + Send + Sync>> + Send>>> {
        let _ = (request_type, request, cx);
        None
    }
}

/// Type-erased system wrapper for runtime storage.
pub trait AnySystem: Send + Sync {
    /// Get the system's display name.
    fn name(&self) -> &'static str;

    /// Get the system's keybinds.
    fn keybinds(&self) -> Keybinds;

    /// Dispatch a handler by ID.
    fn dispatch(&self, handler_id: &HandlerId, cx: &AppContext);

    /// Check if this system has a handler for the given event type.
    fn has_event_handler(&self, event_type: TypeId) -> bool;

    /// Check if this system has a handler for the given request type.
    fn has_request_handler(&self, request_type: TypeId) -> bool;

    /// Dispatch an event to this system's event handlers.
    fn dispatch_event(
        &self,
        event_type: TypeId,
        event: Box<dyn Any + Send + Sync>,
        cx: &AppContext,
    ) -> bool;

    /// Dispatch a request to this system's request handlers.
    fn dispatch_request(
        &self,
        request_type: TypeId,
        request: Box<dyn Any + Send + Sync>,
        cx: &AppContext,
    ) -> Option<Pin<Box<dyn Future<Output = Box<dyn Any + Send + Sync>> + Send>>>;

    /// Clone into a Box.
    fn clone_box(&self) -> Box<dyn AnySystem>;
}

impl<S: System> AnySystem for S {
    fn name(&self) -> &'static str {
        System::name(self)
    }

    fn keybinds(&self) -> Keybinds {
        System::keybinds(self)
    }

    fn dispatch(&self, handler_id: &HandlerId, cx: &AppContext) {
        System::dispatch(self, handler_id, cx);
    }

    fn has_event_handler(&self, event_type: TypeId) -> bool {
        System::has_event_handler(self, event_type)
    }

    fn has_request_handler(&self, request_type: TypeId) -> bool {
        System::has_request_handler(self, request_type)
    }

    fn dispatch_event(
        &self,
        event_type: TypeId,
        event: Box<dyn Any + Send + Sync>,
        cx: &AppContext,
    ) -> bool {
        System::dispatch_event(self, event_type, event, cx)
    }

    fn dispatch_request(
        &self,
        request_type: TypeId,
        request: Box<dyn Any + Send + Sync>,
        cx: &AppContext,
    ) -> Option<Pin<Box<dyn Future<Output = Box<dyn Any + Send + Sync>> + Send>>> {
        System::dispatch_request(self, request_type, request, cx)
    }

    fn clone_box(&self) -> Box<dyn AnySystem> {
        Box::new(self.clone())
    }
}

/// System registration entry for inventory.
pub struct SystemRegistration {
    /// System name.
    pub name: &'static str,
    /// Factory function to create the system.
    pub factory: fn() -> Box<dyn AnySystem>,
}

impl SystemRegistration {
    /// Create a new system registration.
    pub const fn new(name: &'static str, factory: fn() -> Box<dyn AnySystem>) -> Self {
        Self { name, factory }
    }
}

// Collect all registered systems
inventory::collect!(SystemRegistration);

/// Get all registered systems.
pub fn registered_systems() -> impl Iterator<Item = &'static SystemRegistration> {
    inventory::iter::<SystemRegistration>()
}
