//! Modal system for overlay dialogs.
//!
//! Modals are overlay views that capture input until closed.
//! They return a result to the caller.

use std::any::{Any, TypeId};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use tokio::sync::oneshot;
use tuidom::Element;

use crate::{AppContext, GlobalContext};

use crate::{HandlerRegistry, KeybindClosures, LifecycleHooks};

/// Modal position configuration.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ModalPosition {
    /// Centered on screen (default).
    #[default]
    Centered,
    /// Absolute position from top-left corner.
    At { x: u16, y: u16 },
}

/// Modal size configuration.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ModalSize {
    /// Automatically fit content.
    #[default]
    Auto,
    /// Small preset (30% of screen).
    Sm,
    /// Medium preset (50% of screen).
    Md,
    /// Large preset (80% of screen).
    Lg,
    /// Fixed size in cells.
    Fixed { width: u16, height: u16 },
    /// Proportional to screen size (0.0 - 1.0).
    Proportional { width: f32, height: f32 },
}

/// Modal kind - determines what context is available.
///
/// App modals are spawned from within an app and have access to `AppContext`.
/// System modals are spawned globally and only have access to `GlobalContext`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ModalKind {
    /// App-scoped modal with access to `cx`, `gx`, and `mx`.
    #[default]
    App,
    /// System/global modal with access to `gx` and `mx` only.
    System,
}

/// Marker trait for system modals.
///
/// System modals only have access to `GlobalContext` and `ModalContext`,
/// not `AppContext`. Implement this trait (or use `#[modal(kind = System)]`)
/// to indicate a modal is a system modal.
///
/// The macro will enforce at compile time that system modal handlers
/// don't try to use `AppContext`.
pub trait SystemModal: Modal {}

/// Context passed to modal handlers.
///
/// Provides the ability to close the modal and return a result.
pub struct ModalContext<R> {
    result_tx: Arc<Mutex<Option<oneshot::Sender<R>>>>,
    closed: Arc<AtomicBool>,
    focus_request: Arc<Mutex<Option<String>>>,
}

impl<R> ModalContext<R> {
    /// Create a new modal context.
    pub fn new(result_tx: oneshot::Sender<R>) -> Self {
        Self {
            result_tx: Arc::new(Mutex::new(Some(result_tx))),
            closed: Arc::new(AtomicBool::new(false)),
            focus_request: Arc::new(Mutex::new(None)),
        }
    }

    /// Close the modal and return a result.
    pub fn close(&self, result: R) {
        if let Some(tx) = self
            .result_tx
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
        {
            let _ = tx.send(result);
        }
        self.closed.store(true, Ordering::SeqCst);
    }

    /// Check if the modal has been closed.
    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }

    /// Request focus on an element within the modal.
    pub fn focus(&self, element_id: impl Into<String>) {
        if let Ok(mut guard) = self.focus_request.lock() {
            *guard = Some(element_id.into());
        }
    }

    /// Take any pending focus request (for runtime use).
    pub fn take_focus_request(&self) -> Option<String> {
        self.focus_request
            .lock()
            .ok()
            .and_then(|mut guard| guard.take())
    }
}

impl<R> Clone for ModalContext<R> {
    fn clone(&self) -> Self {
        Self {
            result_tx: Arc::clone(&self.result_tx),
            closed: Arc::clone(&self.closed),
            focus_request: Arc::clone(&self.focus_request),
        }
    }
}

unsafe impl<R: Send> Send for ModalContext<R> {}
unsafe impl<R: Send> Sync for ModalContext<R> {}

/// Trait for modal dialogs.
///
/// Typically implemented via `#[modal]` and `#[modal_impl]` macros.
pub trait Modal: Clone + Send + Sync + 'static {
    /// The result type returned when the modal closes.
    type Result: Send + 'static;

    /// The default result to return if the modal is abandoned (e.g., during shutdown).
    ///
    /// This is called by the runtime when closing modals during shutdown.
    /// Must be explicitly implemented - typically returns a "cancelled" or "no-op" value.
    fn default_result(&self) -> Self::Result;

    /// Get the modal's name (for debugging).
    fn name(&self) -> &str {
        std::any::type_name::<Self>()
    }

    /// Get the modal's kind (App or System).
    ///
    /// App modals have access to `AppContext`, `GlobalContext`, and `ModalContext`.
    /// System modals only have access to `GlobalContext` and `ModalContext`.
    fn kind(&self) -> ModalKind {
        ModalKind::default()
    }

    /// Get the modal's position.
    fn position(&self) -> ModalPosition {
        ModalPosition::default()
    }

    /// Get the modal's size.
    fn size(&self) -> ModalSize {
        ModalSize::default()
    }

    /// Get the modal's aspect ratio (width multiplier).
    ///
    /// Applied to Sm/Md/Lg preset sizes. Default is 1.0.
    /// Values < 1.0 make the modal narrower, > 1.0 make it wider.
    fn aspect_ratio(&self) -> f32 {
        1.0
    }

    /// Render the modal's content.
    fn element(&self) -> Element;

    /// Get lifecycle hook closures.
    ///
    /// Override via `#[on_start]` attribute in `#[modal_impl]`.
    fn lifecycle_hooks(&self) -> LifecycleHooks {
        LifecycleHooks::new()
    }

    /// Get the modal's keybinds (closure-based).
    fn keybinds(&self) -> KeybindClosures {
        KeybindClosures::new()
    }

    /// Get the handler registry for widget events.
    fn handlers(&self) -> &HandlerRegistry {
        static EMPTY: OnceLock<HandlerRegistry> = OnceLock::new();
        EMPTY.get_or_init(HandlerRegistry::new)
    }

    /// Check if the modal needs re-rendering.
    fn is_dirty(&self) -> bool {
        true
    }

    /// Clear dirty flags after rendering.
    fn clear_dirty(&self) {}

    /// Check if this modal has a handler for the given event type.
    ///
    /// Override via `#[event_handler]` attribute in `#[modal_impl]`.
    fn has_event_handler(&self, event_type: TypeId) -> bool {
        let _ = event_type;
        false
    }

    /// Dispatch an event to this modal's handlers.
    ///
    /// Override via `#[event_handler]` attribute in `#[modal_impl]`.
    ///
    /// # Parameters
    /// - `event_type`: The TypeId of the event
    /// - `event`: The event data (type-erased)
    /// - `cx`: App context (available for app modals, ignored by system modals)
    /// - `gx`: Global context
    /// - `mx`: Modal context (type-erased, downcast to `ModalContext<Self::Result>` internally)
    ///
    /// # Returns
    /// `true` if the event was handled, `false` otherwise.
    fn dispatch_event(
        &self,
        event_type: TypeId,
        event: &(dyn Any + Send + Sync),
        cx: &AppContext,
        gx: &GlobalContext,
        mx: &(dyn Any + Send + Sync),
    ) -> bool {
        let _ = (event_type, event, cx, gx, mx);
        false
    }
}

// =============================================================================
// ModalEntry (type-erased modal storage)
// =============================================================================

/// Type-erased modal entry for runtime storage.
///
/// Stores a modal and its context in a type-erased form so the runtime
/// can manage modals without knowing their concrete types.
pub struct ModalEntry<M: Modal> {
    /// The modal instance.
    pub modal: M,
    /// The modal context.
    pub context: ModalContext<M::Result>,
}

impl<M: Modal> ModalEntry<M> {
    /// Create a new modal entry.
    pub fn new(modal: M, context: ModalContext<M::Result>) -> Self {
        Self { modal, context }
    }

    /// Check if the modal has been closed.
    pub fn is_closed(&self) -> bool {
        self.context.is_closed()
    }

    /// Render the modal's element.
    pub fn element(&self) -> Element {
        self.modal.element()
    }

    /// Get the modal's keybinds (closure-based).
    pub fn keybinds(&self) -> KeybindClosures {
        self.modal.keybinds()
    }

    /// Get the handler registry for widget events.
    pub fn handlers(&self) -> &HandlerRegistry {
        self.modal.handlers()
    }

    /// Get a reference to the modal context.
    pub fn context(&self) -> &ModalContext<M::Result> {
        &self.context
    }
}
