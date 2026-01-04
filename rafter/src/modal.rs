//! Modal system for overlay dialogs.
//!
//! Modals are overlay views that capture input until closed.
//! They return a result to the caller.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tokio::sync::oneshot;
use tuidom::Element;

use crate::{AppContext, GlobalContext, HandlerId, Keybinds};

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
pub struct ModalContext<R> {
    result_tx: Arc<Mutex<Option<oneshot::Sender<R>>>>,
    closed: Arc<AtomicBool>,
}

impl<R> ModalContext<R> {
    /// Create a new modal context.
    pub fn new(result_tx: oneshot::Sender<R>) -> Self {
        Self {
            result_tx: Arc::new(Mutex::new(Some(result_tx))),
            closed: Arc::new(AtomicBool::new(false)),
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
}

impl<R> Clone for ModalContext<R> {
    fn clone(&self) -> Self {
        Self {
            result_tx: Arc::clone(&self.result_tx),
            closed: Arc::clone(&self.closed),
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

    /// Render the modal's content.
    fn element(&self) -> Element;

    /// Get the modal's keybinds.
    fn keybinds(&self) -> Keybinds {
        Keybinds::new()
    }

    /// Dispatch a handler by ID.
    ///
    /// For app-scoped modals, cx provides access to the parent app's context.
    /// For global modals, cx may be a default/empty context.
    fn dispatch(
        &self,
        handler_id: &HandlerId,
        mx: &ModalContext<Self::Result>,
        cx: &AppContext,
        gx: &GlobalContext,
    ) {
        let _ = (handler_id, mx, cx, gx);
    }

    /// Check if the modal needs re-rendering.
    fn is_dirty(&self) -> bool {
        true
    }

    /// Clear dirty flags after rendering.
    fn clear_dirty(&self) {}
}
