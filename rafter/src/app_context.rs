//! App context for instance-specific operations.
//!
//! AppContext provides access to instance-specific state:
//! - Instance identity
//! - Close self
//! - Focus within the app
//! - Spawn async tasks
//! - App-scoped modals

use std::any::Any;
use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::sync::{Arc, RwLock};

use futures::FutureExt;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

use crate::instance::InstanceId;
use crate::modal::{Modal, ModalContext, ModalEntry};
use crate::wakeup::WakeupSender;
use crate::GlobalContext;

// =============================================================================
// AppError
// =============================================================================

/// Error from an app (panic in handler or task).
#[derive(Debug)]
pub struct AppError {
    /// App name.
    pub app_name: &'static str,
    /// Instance ID.
    pub instance_id: InstanceId,
    /// Error kind.
    pub kind: AppErrorKind,
}

/// Kind of app error.
#[derive(Debug)]
pub enum AppErrorKind {
    /// Handler panicked.
    HandlerPanic { message: String },
    /// Spawned task panicked.
    TaskPanic { message: String },
}

/// Sender for app errors.
pub type ErrorSender = mpsc::UnboundedSender<AppError>;

/// Receiver for app errors.
pub type ErrorReceiver = mpsc::UnboundedReceiver<AppError>;

/// Extract panic message from a panic payload.
pub fn extract_panic_message(panic: &Box<dyn Any + Send>) -> String {
    if let Some(s) = panic.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = panic.downcast_ref::<String>() {
        s.clone()
    } else {
        "Unknown panic".to_string()
    }
}

// =============================================================================
// AppModalRequest
// =============================================================================

/// A request to open an app-scoped modal.
pub struct AppModalRequest {
    /// The type-erased modal entry.
    pub entry: Box<dyn crate::runtime::dispatch::AnyModal>,
}

// =============================================================================
// AppContextInner
// =============================================================================

/// Inner state for AppContext.
struct AppContextInner {
    /// This instance's ID.
    instance_id: InstanceId,
    /// Request to focus a specific element.
    focus_request: Option<String>,
    /// Pending app-scoped modal.
    modal_request: Option<AppModalRequest>,
}

// =============================================================================
// AppContext
// =============================================================================

/// App context for instance-specific operations.
///
/// Passed to app handlers that need instance-specific access.
/// Uses interior mutability - all methods take `&self`.
#[derive(Clone)]
pub struct AppContext {
    inner: Arc<RwLock<AppContextInner>>,
    /// Reference to global context (for close convenience).
    global: GlobalContext,
    /// Wakeup sender for notifying the event loop.
    wakeup_sender: Option<WakeupSender>,
    /// Error sender for reporting panics.
    error_sender: Option<ErrorSender>,
    /// App name (for error reporting).
    app_name: &'static str,
}

impl AppContext {
    /// Create a new app context.
    pub fn new(instance_id: InstanceId, global: GlobalContext, app_name: &'static str) -> Self {
        Self {
            inner: Arc::new(RwLock::new(AppContextInner {
                instance_id,
                focus_request: None,
                modal_request: None,
            })),
            global,
            wakeup_sender: None,
            error_sender: None,
            app_name,
        }
    }

    // =========================================================================
    // Setup (runtime use only)
    // =========================================================================

    /// Set the wakeup sender (called by runtime).
    pub fn set_wakeup_sender(&mut self, sender: WakeupSender) {
        self.wakeup_sender = Some(sender);
    }

    /// Set the global context reference (called by runtime).
    pub fn set_global(&mut self, global: GlobalContext) {
        self.global = global;
    }

    /// Set the error sender (called by runtime).
    pub(crate) fn set_error_sender(&mut self, sender: ErrorSender) {
        self.error_sender = Some(sender);
    }

    /// Send a wakeup signal to the event loop.
    fn send_wakeup(&self) {
        if let Some(sender) = &self.wakeup_sender {
            sender.send();
        }
    }

    /// Report an app error (used internally by panic catching).
    fn report_error(&self, error: AppError) {
        if let Some(sender) = &self.error_sender {
            let _ = sender.send(error);
            self.send_wakeup();
        }
    }

    // =========================================================================
    // Identity
    // =========================================================================

    /// Get this instance's ID.
    pub fn instance_id(&self) -> InstanceId {
        self.inner
            .read()
            .map(|inner| inner.instance_id)
            .unwrap_or_else(|e| e.into_inner().instance_id)
    }

    // =========================================================================
    // Close
    // =========================================================================

    /// Close this instance.
    ///
    /// Convenience method - equivalent to `gx.close(cx.instance_id())`.
    pub fn close(&self) {
        self.global.close(self.instance_id());
    }

    // =========================================================================
    // Focus
    // =========================================================================

    /// Set focus to a specific element by ID.
    ///
    /// The element must be within this app's view tree.
    pub fn focus(&self, element_id: impl Into<String>) {
        if let Ok(mut inner) = self.inner.write() {
            inner.focus_request = Some(element_id.into());
            self.send_wakeup();
        }
    }

    // =========================================================================
    // Spawn Task
    // =========================================================================

    /// Spawn an async task with panic catching.
    ///
    /// The spawned task runs independently and can use cloned state.
    /// If the task panics, the panic is caught and reported to the error handler.
    /// The returned `JoinHandle` yields `Some(value)` on success or `None` on panic.
    ///
    /// # Example
    ///
    /// ```ignore
    /// #[handler]
    /// fn load_data(&self, cx: &AppContext) {
    ///     let data = self.data.clone();
    ///     cx.spawn_task(async move {
    ///         let result = fetch_data().await;
    ///         data.set(result);
    ///     });
    /// }
    /// ```
    pub fn spawn_task<F>(&self, future: F) -> JoinHandle<Option<F::Output>>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        let error_sender = self.error_sender.clone();
        let wakeup_sender = self.wakeup_sender.clone();
        let app_name = self.app_name;
        let instance_id = self.instance_id();

        tokio::spawn(async move {
            let result = AssertUnwindSafe(future).catch_unwind().await;

            match result {
                Ok(value) => Some(value),
                Err(panic) => {
                    let message = extract_panic_message(&panic);
                    if let Some(sender) = error_sender {
                        let _ = sender.send(AppError {
                            app_name,
                            instance_id,
                            kind: AppErrorKind::TaskPanic { message },
                        });
                        if let Some(wakeup) = wakeup_sender {
                            wakeup.send();
                        }
                    }
                    None
                }
            }
        })
    }

    // =========================================================================
    // App-Scoped Modal
    // =========================================================================

    /// Open an app-scoped modal and wait for it to return a result.
    ///
    /// App-scoped modals are tied to this instance and are hidden when
    /// the app loses focus. Use `gx.modal()` for global modals that
    /// overlay everything.
    pub async fn modal<M: Modal>(&self, modal: M) -> M::Result {
        let (tx, rx) = oneshot::channel();

        // Create the modal context with the result sender
        let mx = ModalContext::new(tx);

        // Create the type-erased entry
        let entry = ModalEntry::new(modal, mx);

        // Request the runtime to push this modal
        if let Ok(mut inner) = self.inner.write() {
            inner.modal_request = Some(AppModalRequest {
                entry: Box::new(entry),
            });
            self.send_wakeup();
        }

        // Wait for the modal to close and return its result
        rx.await.expect("modal closed without sending result")
    }

    // =========================================================================
    // Internal (runtime use)
    // =========================================================================

    /// Take the focus request (runtime use).
    pub(crate) fn take_focus_request(&self) -> Option<String> {
        self.inner
            .write()
            .ok()
            .and_then(|mut inner| inner.focus_request.take())
    }

    /// Take the modal request (runtime use).
    pub(crate) fn take_modal_request(&self) -> Option<AppModalRequest> {
        self.inner
            .write()
            .ok()
            .and_then(|mut inner| inner.modal_request.take())
    }
}

impl Default for AppContext {
    fn default() -> Self {
        Self {
            inner: Arc::new(RwLock::new(AppContextInner {
                instance_id: InstanceId::default(),
                focus_request: None,
                modal_request: None,
            })),
            global: GlobalContext::default(),
            wakeup_sender: None,
            error_sender: None,
            app_name: "unknown",
        }
    }
}
