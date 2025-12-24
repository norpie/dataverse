//! Modal system for Rafter.
//!
//! Modals are overlay views that capture input until closed. They can be used for
//! confirmations, forms, or full-featured sub-applications.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tokio::sync::oneshot;

use crate::context::AppContext;
use crate::runtime::wakeup;
use crate::input::keybinds::{HandlerId, Keybinds};
use crate::node::Node;

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

/// Context passed to modal handlers.
///
/// Provides the ability to close the modal and return a result.
/// The type parameter `R` is the result type that will be returned
/// to the caller of `cx.modal(...)`.
pub struct ModalContext<R> {
    result_tx: Arc<Mutex<Option<oneshot::Sender<R>>>>,
    closed: Arc<AtomicBool>,
}

impl<R> ModalContext<R> {
    /// Create a new modal context with the given result sender.
    pub(crate) fn new(result_tx: oneshot::Sender<R>) -> Self {
        Self {
            result_tx: Arc::new(Mutex::new(Some(result_tx))),
            closed: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Close the modal and return a result.
    ///
    /// This sends the result back to the caller that opened the modal
    /// and marks the modal for removal from the stack.
    ///
    /// # Example
    ///
    /// ```ignore
    /// #[handler]
    /// async fn confirm(&self, mx: &ModalContext<bool>) {
    ///     mx.close(true);
    /// }
    /// ```
    pub fn close(&self, result: R) {
        if let Some(tx) = self.result_tx.lock().unwrap().take() {
            let _ = tx.send(result);
        }
        self.closed.store(true, Ordering::SeqCst);
        log::debug!("ModalContext::close() sending wakeup");
        wakeup::send_wakeup();
    }

    /// Check if the modal has been closed.
    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }
}

impl<R> Clone for ModalContext<R> {
    fn clone(&self) -> Self {
        Self {
            result_tx: Arc::clone(&self.result_tx),
            closed: Arc::clone(&self.closed),
        }
    }
}

// Safety: ModalContext is Send + Sync because:
// - Arc<Mutex<...>> is Send + Sync
// - Arc<AtomicBool> is Send + Sync
unsafe impl<R: Send> Send for ModalContext<R> {}
unsafe impl<R: Send> Sync for ModalContext<R> {}

/// Trait for modal types with a known result type.
///
/// This trait is implemented by the `#[modal_impl]` macro.
pub trait Modal: Clone + Send + Sync + 'static {
    /// The result type returned when the modal closes.
    type Result: Send + 'static;

    /// Get the modal's name (for debugging).
    fn name(&self) -> &str {
        std::any::type_name::<Self>()
    }

    /// Get the modal's position.
    fn position(&self) -> ModalPosition {
        ModalPosition::default()
    }

    /// Get the modal's size.
    fn size(&self) -> ModalSize {
        ModalSize::default()
    }

    /// Render the modal's page.
    fn page(&self) -> Node;

    /// Get the modal's keybinds.
    fn keybinds(&self) -> Keybinds;

    /// Dispatch a handler by ID.
    fn dispatch(&self, handler_id: &HandlerId, cx: &AppContext, mx: &ModalContext<Self::Result>);

    /// Check if the modal needs re-rendering.
    fn is_dirty(&self) -> bool {
        true // Default: always re-render
    }

    /// Clear dirty flags after rendering.
    fn clear_dirty(&self);
}

/// Type-erased modal trait for runtime storage.
///
/// This allows the runtime to store modals of different result types
/// in the same stack.
pub(crate) trait ModalDyn: Send + Sync {
    /// Get the modal's name.
    fn name(&self) -> &str;

    /// Get the modal's position.
    fn position(&self) -> ModalPosition;

    /// Get the modal's size.
    fn size(&self) -> ModalSize;

    /// Render the modal's page.
    fn page(&self) -> Node;

    /// Get the modal's keybinds.
    fn keybinds(&self) -> Keybinds;

    /// Dispatch a handler (type-erased).
    fn dispatch_dyn(&self, handler_id: &HandlerId, cx: &AppContext);

    /// Check if the modal needs re-rendering.
    fn is_dirty(&self) -> bool;

    /// Clear dirty flags.
    fn clear_dirty(&self);

    /// Check if the modal has been closed.
    fn is_closed(&self) -> bool;
}

/// Wrapper that holds a modal and its context together for type erasure.
pub(crate) struct ModalEntry<M: Modal> {
    modal: M,
    context: ModalContext<M::Result>,
}

impl<M: Modal> ModalEntry<M> {
    /// Create a new modal entry.
    pub fn new(modal: M, context: ModalContext<M::Result>) -> Self {
        Self { modal, context }
    }
}

impl<M: Modal> ModalDyn for ModalEntry<M> {
    fn name(&self) -> &str {
        self.modal.name()
    }

    fn position(&self) -> ModalPosition {
        self.modal.position()
    }

    fn size(&self) -> ModalSize {
        self.modal.size()
    }

    fn page(&self) -> Node {
        self.modal.page()
    }

    fn keybinds(&self) -> Keybinds {
        self.modal.keybinds()
    }

    fn dispatch_dyn(&self, handler_id: &HandlerId, cx: &AppContext) {
        self.modal.dispatch(handler_id, cx, &self.context);
    }

    fn is_dirty(&self) -> bool {
        self.modal.is_dirty()
    }

    fn clear_dirty(&self) {
        self.modal.clear_dirty();
    }

    fn is_closed(&self) -> bool {
        self.context.is_closed()
    }
}
