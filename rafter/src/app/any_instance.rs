//! Type-erased app instance wrapper for runtime storage.

use std::any::{Any, TypeId};
use std::future::Future;
use std::pin::Pin;

use crate::context::AppContext;
use crate::input::keybinds::{HandlerId, KeyCombo, Keybinds};
use crate::node::Node;
use crate::widgets::events::EventResult;

use super::config::AppConfig;
use super::instance::{InstanceId, InstanceInfo};
use super::traits::App;

/// Type-erased wrapper for app instances in the registry.
///
/// This trait allows the runtime to store and operate on app instances
/// without knowing their concrete types.
pub trait AnyAppInstance: Send + Sync {
    /// Get the unique instance ID.
    fn id(&self) -> InstanceId;

    /// Get the TypeId of the underlying app type.
    fn type_id(&self) -> TypeId;

    /// Get instance metadata for app switcher UI.
    fn info(&self) -> InstanceInfo;

    /// Get the app configuration.
    fn config(&self) -> AppConfig;

    /// Get the app's keybinds.
    fn keybinds(&self) -> Keybinds;

    /// Get the current page identifier for keybind scoping.
    fn current_page(&self) -> Option<String>;

    // Lifecycle methods

    /// Called when the app instance is first started.
    fn on_start(&self, cx: &AppContext);

    /// Called when instance gains focus.
    fn on_foreground(&self, cx: &AppContext);

    /// Called when instance loses focus.
    fn on_background(&self, cx: &AppContext);

    /// Called before close. Returns false to cancel.
    fn on_close_request(&self, cx: &AppContext) -> bool;

    /// Called during cleanup after close.
    fn on_close(&self, cx: &AppContext);

    // Persistence (returns None if not PersistentApp)

    /// Serialize state for session persistence.
    /// Returns None if the app doesn't implement PersistentApp.
    fn save_state(&self) -> Option<Vec<u8>>;

    // Rendering & events

    /// Render the app's page tree.
    fn page(&self) -> Node;

    /// Dispatch a keybind to the app.
    fn dispatch_key(&self, key: &KeyCombo, cx: &AppContext) -> EventResult;

    /// Dispatch a handler by ID.
    fn dispatch(&self, handler_id: &HandlerId, cx: &AppContext);

    /// Check if the app needs re-rendering.
    fn is_dirty(&self) -> bool;

    /// Clear dirty flags after render.
    fn clear_dirty(&self);

    /// Clone the instance into a new box.
    fn clone_box(&self) -> Box<dyn AnyAppInstance>;

    /// Update the last focused time.
    fn mark_focused(&mut self);

    /// Update focused state.
    fn set_focused(&mut self, focused: bool);

    /// Check if the instance is sleeping.
    fn is_sleeping(&self) -> bool;

    /// Set the sleeping state.
    fn set_sleeping(&mut self, sleeping: bool);

    // Event/Request dispatch

    /// Check if this instance has a handler for the given event type.
    fn has_event_handler(&self, event_type: TypeId) -> bool;

    /// Check if this instance has a handler for the given request type.
    fn has_request_handler(&self, request_type: TypeId) -> bool;

    /// Dispatch an event to this instance's handlers.
    ///
    /// Returns true if a handler was found and invoked.
    /// The handler runs in a spawned task (fire-and-forget).
    fn dispatch_event(&self, event_type: TypeId, event: Box<dyn Any + Send + Sync>, cx: &AppContext) -> bool;

    /// Dispatch a request to this instance's handlers.
    ///
    /// Returns Some(future) if a handler exists for the request type.
    /// The future resolves to the response.
    fn dispatch_request(
        &self,
        request_type: TypeId,
        request: Box<dyn Any + Send + Sync>,
        cx: &AppContext,
    ) -> Option<Pin<Box<dyn Future<Output = Box<dyn Any + Send + Sync>> + Send>>>;
}

/// Wrapper that implements AnyAppInstance for any App.
pub struct AppInstance<A: App> {
    /// The app instance.
    app: A,
    /// Unique instance ID.
    id: InstanceId,
    /// Instance info.
    info: InstanceInfo,
}

impl<A: App> AppInstance<A> {
    /// Create a new app instance.
    pub fn new(app: A) -> Self {
        let id = InstanceId::new();
        let config = A::config();
        let title = app.title();
        let info = InstanceInfo::new(id, config.name, title);

        Self { app, id, info }
    }

    /// Get a reference to the underlying app.
    pub fn app(&self) -> &A {
        &self.app
    }

    /// Get a mutable reference to the underlying app.
    pub fn app_mut(&mut self) -> &mut A {
        &mut self.app
    }
}

impl<A: App> AnyAppInstance for AppInstance<A> {
    fn id(&self) -> InstanceId {
        self.id
    }

    fn type_id(&self) -> TypeId {
        TypeId::of::<A>()
    }

    fn info(&self) -> InstanceInfo {
        // Update title in case it changed
        let mut info = self.info.clone();
        info.title = self.app.title();
        info
    }

    fn config(&self) -> AppConfig {
        A::config()
    }

    fn keybinds(&self) -> Keybinds {
        self.app.keybinds()
    }

    fn current_page(&self) -> Option<String> {
        self.app.current_page()
    }

    fn on_start(&self, cx: &AppContext) {
        let app = self.app.clone();
        let cx = cx.clone();
        tokio::spawn(async move {
            app.on_start(&cx).await;
        });
    }

    fn on_foreground(&self, cx: &AppContext) {
        // We need to spawn the future since the trait method returns impl Future
        let app = self.app.clone();
        let cx = cx.clone();
        tokio::spawn(async move {
            app.on_foreground(&cx).await;
        });
    }

    fn on_background(&self, cx: &AppContext) {
        let app = self.app.clone();
        let cx = cx.clone();
        tokio::spawn(async move {
            app.on_background(&cx).await;
        });
    }

    fn on_close_request(&self, cx: &AppContext) -> bool {
        self.app.on_close_request(cx)
    }

    fn on_close(&self, cx: &AppContext) {
        let app = self.app.clone();
        let cx = cx.clone();
        tokio::spawn(async move {
            app.on_close(&cx).await;
        });
    }

    fn save_state(&self) -> Option<Vec<u8>> {
        // Default: not persistent
        // Override in specialized impl for PersistentApp
        None
    }

    fn page(&self) -> Node {
        self.app.page()
    }

    fn dispatch_key(&self, key: &KeyCombo, cx: &AppContext) -> EventResult {
        let keybinds = self.app.keybinds();
        let current_page = self.app.current_page();

        if let Some(handler_id) = keybinds.get_single(key, current_page.as_deref()) {
            self.app.dispatch(handler_id, cx);
            EventResult::Consumed
        } else {
            EventResult::Ignored
        }
    }

    fn dispatch(&self, handler_id: &HandlerId, cx: &AppContext) {
        self.app.dispatch(handler_id, cx);
    }

    fn is_dirty(&self) -> bool {
        self.app.is_dirty()
    }

    fn clear_dirty(&self) {
        self.app.clear_dirty();
    }

    fn clone_box(&self) -> Box<dyn AnyAppInstance> {
        Box::new(Self {
            app: self.app.clone(),
            id: self.id,
            info: self.info.clone(),
        })
    }

    fn mark_focused(&mut self) {
        self.info.last_focused_at = std::time::Instant::now();
    }

    fn set_focused(&mut self, focused: bool) {
        self.info.is_focused = focused;
    }

    fn is_sleeping(&self) -> bool {
        self.info.is_sleeping
    }

    fn set_sleeping(&mut self, sleeping: bool) {
        self.info.is_sleeping = sleeping;
    }

    fn has_event_handler(&self, event_type: TypeId) -> bool {
        self.app.has_event_handler(event_type)
    }

    fn has_request_handler(&self, request_type: TypeId) -> bool {
        self.app.has_request_handler(request_type)
    }

    fn dispatch_event(&self, event_type: TypeId, event: Box<dyn Any + Send + Sync>, cx: &AppContext) -> bool {
        self.app.dispatch_event(event_type, event, cx)
    }

    fn dispatch_request(
        &self,
        request_type: TypeId,
        request: Box<dyn Any + Send + Sync>,
        cx: &AppContext,
    ) -> Option<Pin<Box<dyn Future<Output = Box<dyn Any + Send + Sync>> + Send>>> {
        self.app.dispatch_request(request_type, request, cx)
    }
}

impl<A: App> Clone for AppInstance<A> {
    fn clone(&self) -> Self {
        Self {
            app: self.app.clone(),
            id: self.id,
            info: self.info.clone(),
        }
    }
}
