//! App trait definitions.

use std::any::{Any, TypeId};
use std::future::Future;
use std::pin::Pin;

use serde::{Serialize, de::DeserializeOwned};

use crate::context::AppContext;
use crate::input::keybinds::{HandlerId, Keybinds};
use crate::node::Node;

use super::config::AppConfig;

/// Panic behavior for an app
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PanicBehavior {
    /// Show error to user, app continues in degraded state
    #[default]
    ShowError,
    /// Kill app and restart fresh
    RestartApp,
    /// Propagate panic, crash the runtime
    CrashRuntime,
}

/// Trait that all apps must implement.
///
/// This is typically implemented via the `#[app]` and `#[app_impl]` macros.
///
/// Apps use interior mutability - all methods take `&self`, and state is
/// wrapped in `State<T>` or `Resource<T>` which provide thread-safe access.
///
/// # Example
///
/// ```ignore
/// #[app]
/// struct Counter {
///     value: i32,  // Becomes State<i32>
/// }
///
/// #[app_impl]
/// impl Counter {
///     #[handler]
///     async fn increment(&self, cx: &AppContext) {
///         self.value.update(|v| *v += 1);
///     }
///
///     fn page(&self) -> Node {
///         let val = self.value.get();
///         page! { text { val.to_string() } }
///     }
/// }
/// ```
pub trait App: Clone + Send + Sync + 'static {
    /// Get the app-level configuration.
    ///
    /// This defines the display name, blur policy, persistence, and instance limits.
    /// Override this to customize app behavior.
    fn config() -> AppConfig
    where
        Self: Sized,
    {
        AppConfig::default()
    }

    /// Get the app's display name.
    ///
    /// This is a convenience method that returns the name from config.
    fn name(&self) -> &'static str {
        Self::config().name
    }

    /// Get the instance-specific title.
    ///
    /// Override this to provide a dynamic title (e.g., "Record #123").
    /// By default, returns the app's config name.
    fn title(&self) -> String {
        Self::config().name.to_string()
    }

    /// Get the app's keybinds
    fn keybinds(&self) -> Keybinds {
        Keybinds::new()
    }

    /// Render the app's page.
    ///
    /// The page tree is used by the runtime to:
    /// - Determine focusable elements (buttons, inputs)
    /// - Get handler references (on_click, on_change, on_submit)
    /// - Sync input values when focus changes
    fn page(&self) -> Node;

    /// Get the current page identifier for keybind scoping.
    ///
    /// Return `Some(name)` to enable page-scoped keybinds.
    /// Keybinds marked with `#[keybinds(page = X)]` will only be active
    /// when this method returns a string matching `X`.
    ///
    /// The recommended pattern is to use an enum that implements `Display`:
    ///
    /// ```ignore
    /// #[derive(strum::Display)]
    /// enum Page { List, Record }
    ///
    /// #[app]
    /// struct MyApp {
    ///     page: Page,
    /// }
    ///
    /// #[app_impl]
    /// impl MyApp {
    ///     fn current_page(&self) -> Option<String> {
    ///         Some(self.page.get().to_string())
    ///     }
    /// }
    /// ```
    fn current_page(&self) -> Option<String> {
        None
    }

    /// Called when the app starts.
    fn on_start(&self, cx: &AppContext) -> impl Future<Output = ()> + Send {
        let _ = cx;
        async {}
    }

    /// Called when the instance gains focus.
    fn on_foreground(&self, cx: &AppContext) -> impl Future<Output = ()> + Send {
        let _ = cx;
        async {}
    }

    /// Called when the instance loses focus.
    fn on_background(&self, cx: &AppContext) -> impl Future<Output = ()> + Send {
        let _ = cx;
        async {}
    }

    /// Called before close. Return false to cancel (e.g., unsaved changes).
    ///
    /// This is called when `cx.close(id)` is used. If it returns false,
    /// the close is cancelled. `cx.force_close(id)` skips this check.
    fn on_close_request(&self, cx: &AppContext) -> bool {
        let _ = cx;
        true
    }

    /// Called during cleanup after close is confirmed.
    ///
    /// Use this for final cleanup like saving state or releasing resources.
    fn on_close(&self, cx: &AppContext) -> impl Future<Output = ()> + Send {
        let _ = cx;
        async {}
    }

    /// Called when the app is about to stop (legacy alias for on_close).
    fn on_stop(&self, cx: &AppContext) -> impl Future<Output = ()> + Send {
        let _ = cx;
        async {}
    }

    /// Get the panic behavior for this app
    fn panic_behavior(&self) -> PanicBehavior {
        PanicBehavior::default()
    }

    /// Check if any state is dirty and needs re-render
    fn is_dirty(&self) -> bool {
        true // Default: always re-render (conservative)
    }

    /// Clear dirty flags after render
    fn clear_dirty(&self) {}

    /// Install wakeup sender on all State fields.
    ///
    /// Called by the runtime when the app is registered.
    fn install_wakeup(&self, _sender: crate::runtime::wakeup::WakeupSender) {
        // Default: no State fields
        // Generated by #[app] macro for apps with State fields
    }

    /// Dispatch a handler by ID.
    ///
    /// This is called by the runtime when a keybind is matched or a UI element
    /// is activated. The implementation spawns an async task to run the handler.
    fn dispatch(&self, handler_id: &HandlerId, cx: &AppContext) {
        // Default implementation does nothing.
        // Generated by #[app_impl] macro.
        log::warn!(
            "Default dispatch called for handler '{}' - macro may not have generated dispatch",
            handler_id.0
        );
        let _ = cx; // Suppress unused warning
    }

    /// Dispatch an event to this app's event handlers.
    ///
    /// Returns true if the event was handled (a handler exists for this event type).
    /// The handler runs in a spawned task and doesn't block.
    fn dispatch_event(&self, event_type: TypeId, event: Box<dyn Any + Send + Sync>, cx: &AppContext) -> bool {
        // Default: no event handlers
        let _ = (event_type, event, cx);
        false
    }

    /// Dispatch a request to this app's request handlers.
    ///
    /// Returns Some(future) if a handler exists for this request type.
    /// The future resolves to the response wrapped in Box<dyn Any + Send + Sync>.
    fn dispatch_request(
        &self,
        request_type: TypeId,
        request: Box<dyn Any + Send + Sync>,
        cx: &AppContext,
    ) -> Option<Pin<Box<dyn Future<Output = Box<dyn Any + Send + Sync>> + Send>>> {
        // Default: no request handlers
        let _ = (request_type, request, cx);
        None
    }

    /// Check if this app has a handler for the given event type.
    fn has_event_handler(&self, event_type: TypeId) -> bool {
        let _ = event_type;
        false
    }

    /// Check if this app has a handler for the given request type.
    fn has_request_handler(&self, request_type: TypeId) -> bool {
        let _ = request_type;
        false
    }
}

/// Trait for apps that support session persistence and recovery.
///
/// Implement this trait to allow your app's state to be saved when the
/// application closes and restored when it starts again.
///
/// # Example
///
/// ```ignore
/// #[derive(Serialize, Deserialize)]
/// struct ViewerState {
///     record_id: RecordId,
///     scroll_position: usize,
/// }
///
/// impl PersistentApp for RecordViewerApp {
///     type SaveState = ViewerState;
///
///     fn save_state(&self) -> ViewerState {
///         ViewerState {
///             record_id: self.record_id.get(),
///             scroll_position: self.scroll_position.get(),
///         }
///     }
///
///     fn restore(state: ViewerState) -> Self {
///         Self {
///             record_id: State::new(state.record_id),
///             scroll_position: State::new(state.scroll_position),
///         }
///     }
/// }
/// ```
pub trait PersistentApp: App {
    /// The state type to serialize for session recovery.
    ///
    /// This should contain all the information needed to reconstruct
    /// the app instance, including any arguments that were passed to create it.
    type SaveState: Serialize + DeserializeOwned + Send + 'static;

    /// Extract current state for saving.
    ///
    /// Called when creating a session snapshot.
    fn save_state(&self) -> Self::SaveState;

    /// Create an instance from saved state.
    ///
    /// Called when restoring a session.
    fn restore(state: Self::SaveState) -> Self
    where
        Self: Sized;
}
