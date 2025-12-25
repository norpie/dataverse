use std::any::{Any, TypeId};
use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use futures::FutureExt;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

use crate::app::{
    AnyAppInstance, App, AppError, AppErrorKind, InstanceId, InstanceInfo, InstanceRegistry,
    SpawnError, extract_panic_message,
};
use crate::event::Event;
use crate::input::focus::FocusId;
use crate::input::keybinds::{KeybindError, KeybindInfo, Keybinds};
use crate::layers::modal::{Modal, ModalContext, ModalDyn, ModalEntry};
use crate::request::RequestError;
use crate::runtime::wakeup;
use crate::runtime::DataStore;
use crate::styling::theme::Theme;
use crate::widgets::events::WidgetEvent;

/// Wrapper that allows cloning boxed events.
///
/// Since Event requires Clone, we store a clone function alongside the event.
pub struct CloneableEvent {
    event: Box<dyn Any + Send + Sync>,
    clone_fn: fn(&Box<dyn Any + Send + Sync>) -> Box<dyn Any + Send + Sync>,
}

impl CloneableEvent {
    /// Create a new cloneable event.
    pub fn new<E: Event>(event: E) -> Self {
        Self {
            event: Box::new(event),
            clone_fn: |e| {
                let e = e.downcast_ref::<E>().expect("type mismatch in CloneableEvent");
                Box::new(e.clone())
            },
        }
    }

    /// Get the event type ID.
    pub fn type_id(&self) -> TypeId {
        (*self.event).type_id()
    }

    /// Take the inner boxed event (consumes self).
    pub fn into_inner(self) -> Box<dyn Any + Send + Sync> {
        self.event
    }
}

impl Clone for CloneableEvent {
    fn clone(&self) -> Self {
        Self {
            event: (self.clone_fn)(&self.event),
            clone_fn: self.clone_fn,
        }
    }
}

/// Toast notification level
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ToastLevel {
    #[default]
    Info,
    Success,
    Warning,
    Error,
}

/// A toast notification
#[derive(Debug, Clone)]
pub struct Toast {
    /// Title to display (single line)
    pub title: String,
    /// Optional body text (can be multi-line)
    pub body: Option<String>,
    /// Toast level (affects styling)
    pub level: ToastLevel,
    /// How long to show the toast
    pub duration: Duration,
}

impl Toast {
    /// Create a simple info toast
    pub fn info(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            body: None,
            level: ToastLevel::Info,
            duration: Duration::from_secs(3),
        }
    }

    /// Create an error toast
    pub fn error(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            body: None,
            level: ToastLevel::Error,
            duration: Duration::from_secs(5),
        }
    }

    /// Create a success toast
    pub fn success(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            body: None,
            level: ToastLevel::Success,
            duration: Duration::from_secs(3),
        }
    }

    /// Create a warning toast
    pub fn warning(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            body: None,
            level: ToastLevel::Warning,
            duration: Duration::from_secs(4),
        }
    }

    /// Add a body to the toast
    pub fn with_body(mut self, body: impl Into<String>) -> Self {
        self.body = Some(body.into());
        self
    }

    /// Set custom duration
    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }

    /// Calculate the height of this toast in lines
    pub fn height(&self, max_width: u16) -> u16 {
        let mut height = 1; // Title line
        if let Some(body) = &self.body {
            // Count wrapped lines for body
            let body_width = max_width.saturating_sub(3) as usize; // Account for padding
            if body_width > 0 {
                for line in body.lines() {
                    height += ((line.len() + body_width - 1) / body_width).max(1) as u16;
                }
            }
        }
        height
    }
}

impl From<String> for Toast {
    fn from(message: String) -> Self {
        Toast::info(message)
    }
}

impl From<&str> for Toast {
    fn from(message: &str) -> Self {
        Toast::info(message)
    }
}

/// Target for a request.
#[derive(Debug, Clone)]
pub enum RequestTarget {
    /// Target the first (non-sleeping) instance of an app type.
    AppType(TypeId),
    /// Target a specific instance by ID.
    Instance(InstanceId),
}

/// Command to manage app instances.
///
/// These commands are queued and processed by the runtime event loop.
pub enum InstanceCommand {
    /// Spawn a new app instance.
    Spawn {
        /// The boxed app instance to spawn.
        instance: Box<dyn AnyAppInstance>,
        /// Whether to focus the new instance.
        focus: bool,
    },
    /// Close an instance.
    Close {
        /// The instance to close.
        id: InstanceId,
        /// Whether to force close (skip on_close_request).
        force: bool,
    },
    /// Focus an instance.
    Focus {
        /// The instance to focus.
        id: InstanceId,
    },
    /// Publish an event to all non-sleeping instances.
    PublishEvent {
        /// The cloneable event to publish.
        event: CloneableEvent,
    },
    /// Send a request to an instance.
    SendRequest {
        /// The target of the request.
        target: RequestTarget,
        /// The request to send.
        request: Box<dyn Any + Send + Sync>,
        /// The TypeId of the request.
        request_type: TypeId,
        /// Channel to send the response back.
        response_tx: oneshot::Sender<Result<Box<dyn Any + Send + Sync>, RequestError>>,
    },
}

/// Inner state for AppContext
struct AppContextInner {
    /// Request to exit the app
    exit_requested: bool,
    /// Request to focus a specific element
    focus_request: Option<FocusId>,
    /// Pending toasts to show
    pending_toasts: Vec<Toast>,
    /// Text input from the current input event
    input_text: Option<String>,
    /// Request to change theme
    theme_request: Option<Arc<dyn Theme>>,
    /// Pending modal to open
    modal_request: Option<Box<dyn ModalDyn>>,

    // -------------------------------------------------------------------------
    // Instance management
    // -------------------------------------------------------------------------
    /// Pending instance commands to process
    instance_commands: Vec<InstanceCommand>,
    /// Current instance ID (set by runtime)
    current_instance_id: Option<InstanceId>,

    // -------------------------------------------------------------------------
    // Unified widget event queue and data
    // -------------------------------------------------------------------------
    /// Pending widget events to dispatch
    pending_events: Vec<WidgetEvent>,

    /// Activated item ID (works for list/tree/table)
    activated_id: Option<String>,
    /// Activated item index (for list - index-based access)
    activated_index: Option<usize>,
    /// Selected item IDs (works for list/tree/table)
    selected_ids: Option<Vec<String>>,
    /// Cursor item ID (works for list/tree/table)
    cursor_id: Option<String>,
    /// Cursor index (for list - index-based access)
    cursor_index: Option<usize>,
    /// Expanded node ID (for tree)
    expanded_id: Option<String>,
    /// Collapsed node ID (for tree)
    collapsed_id: Option<String>,
    /// Sorted column info (column index, ascending) (for table)
    sorted_column: Option<(usize, bool)>,
}

/// Context passed to app handlers, providing access to framework functionality.
///
/// `AppContext` uses interior mutability, so all methods take `&self`.
/// This allows it to be cloned and used across async boundaries.
///
/// # Example
///
/// ```ignore
/// #[handler]
/// async fn my_handler(&self, cx: &AppContext) {
///     cx.toast("Hello!");
///     
///     // Can be used across .await points
///     some_async_operation().await;
///     
///     cx.toast("Done!");
/// }
/// ```
/// Sender for app errors (panics, task failures).
pub type ErrorSender = mpsc::UnboundedSender<AppError>;

/// Receiver for app errors (panics, task failures).
pub type ErrorReceiver = mpsc::UnboundedReceiver<AppError>;

#[derive(Clone)]
pub struct AppContext {
    inner: Arc<RwLock<AppContextInner>>,
    /// Shared keybinds (can be modified at runtime)
    keybinds: Arc<RwLock<Keybinds>>,
    /// Shared instance registry for querying instances
    registry: Option<Arc<RwLock<InstanceRegistry>>>,
    /// Global data store (read-only, set at runtime startup)
    data: Arc<DataStore>,
    /// Wakeup sender for notifying the event loop (works across threads)
    wakeup_sender: Option<wakeup::WakeupSender>,
    /// Error sender for reporting app errors (panics, task failures)
    error_sender: Option<ErrorSender>,
}

impl AppContext {
    /// Create a new app context with shared keybinds and data store
    pub fn new(keybinds: Arc<RwLock<Keybinds>>, data: Arc<DataStore>) -> Self {
        Self {
            inner: Arc::new(RwLock::new(AppContextInner {
                exit_requested: false,
                focus_request: None,
                pending_toasts: Vec::new(),
                input_text: None,
                theme_request: None,
                modal_request: None,
                instance_commands: Vec::new(),
                current_instance_id: None,
                pending_events: Vec::new(),
                activated_id: None,
                activated_index: None,
                selected_ids: None,
                cursor_id: None,
                cursor_index: None,
                expanded_id: None,
                collapsed_id: None,
                sorted_column: None,
            })),
            keybinds,
            registry: None,
            data,
            wakeup_sender: None,
            error_sender: None,
        }
    }

    /// Set the wakeup sender (called by runtime)
    pub(crate) fn set_wakeup_sender(&mut self, sender: wakeup::WakeupSender) {
        self.wakeup_sender = Some(sender);
    }

    /// Set the error sender (called by runtime)
    pub(crate) fn set_error_sender(&mut self, sender: ErrorSender) {
        self.error_sender = Some(sender);
    }

    /// Report an app error (used internally by panic catching).
    ///
    /// This sends the error to the event loop for handling according to
    /// the app's panic behavior policy.
    pub fn report_error(&self, error: AppError) {
        if let Some(sender) = &self.error_sender {
            // Ignore send errors - the receiver might be gone during shutdown
            let _ = sender.send(error);
            // Wake up the event loop to process the error
            self.send_wakeup();
        }
    }

    /// Send a wakeup signal to the event loop
    fn send_wakeup(&self) {
        if let Some(sender) = &self.wakeup_sender {
            sender.send();
        }
    }

    // -------------------------------------------------------------------------
    // Global data access
    // -------------------------------------------------------------------------

    /// Get a reference to global data of type `T`.
    ///
    /// Returns `None` if no data of this type was registered with `Runtime::data()`.
    ///
    /// # Example
    ///
    /// ```ignore
    /// if let Some(client) = cx.try_data::<ApiClient>() {
    ///     client.fetch().await;
    /// }
    /// ```
    pub fn try_data<T: Send + Sync + 'static>(&self) -> Option<&T> {
        self.data
            .get(&TypeId::of::<T>())
            .and_then(|arc| arc.downcast_ref::<T>())
    }

    /// Get a reference to global data of type `T`.
    ///
    /// # Panics
    ///
    /// Panics if no data of this type was registered with `Runtime::data()`.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let client = cx.data::<ApiClient>();
    /// let result = client.fetch().await;
    /// ```
    pub fn data<T: Send + Sync + 'static>(&self) -> &T {
        self.try_data::<T>().expect(&format!(
            "No data of type {} registered. Use Runtime::data() to register it.",
            std::any::type_name::<T>()
        ))
    }

    /// Request to exit the current app
    pub fn exit(&self) {
        log::debug!("cx.exit() called");
        if let Ok(mut inner) = self.inner.write() {
            inner.exit_requested = true;
            log::debug!("AppContext: sending wakeup (exit)");
            self.send_wakeup();
        }
    }

    /// Set focus to a specific element by ID
    pub fn focus(&self, id: impl Into<FocusId>) {
        if let Ok(mut inner) = self.inner.write() {
            inner.focus_request = Some(id.into());
            log::debug!("AppContext: sending wakeup (focus)");
            self.send_wakeup();
        }
    }

    /// Show a toast notification.
    ///
    /// Accepts either a string (creates an info toast) or a Toast directly.
    ///
    /// # Examples
    /// ```ignore
    /// // Simple string toast
    /// cx.toast("Operation completed");
    ///
    /// // Toast with body
    /// cx.toast(Toast::success("File saved")
    ///     .with_body("Your changes have been saved."));
    /// ```
    pub fn toast(&self, toast: impl Into<Toast>) {
        if let Ok(mut inner) = self.inner.write() {
            inner.pending_toasts.push(toast.into());
            log::debug!("AppContext: sending wakeup (toast)");
            self.send_wakeup();
        }
    }

    /// Show an error toast notification
    #[deprecated(note = "Use cx.toast(Toast::error(...)) instead")]
    pub fn toast_error(&self, message: impl Into<String>) {
        if let Ok(mut inner) = self.inner.write() {
            inner.pending_toasts.push(Toast::error(message));
            log::debug!("AppContext: sending wakeup (toast_error)");
            self.send_wakeup();
        }
    }

    /// Show a success toast notification
    #[deprecated(note = "Use cx.toast(Toast::success(...)) instead")]
    pub fn toast_success(&self, message: impl Into<String>) {
        if let Ok(mut inner) = self.inner.write() {
            inner.pending_toasts.push(Toast::success(message));
            log::debug!("AppContext: sending wakeup (toast_success)");
            self.send_wakeup();
        }
    }

    /// Show a warning toast notification
    #[deprecated(note = "Use cx.toast(Toast::warning(...)) instead")]
    pub fn toast_warning(&self, message: impl Into<String>) {
        if let Ok(mut inner) = self.inner.write() {
            inner.pending_toasts.push(Toast::warning(message));
            log::debug!("AppContext: sending wakeup (toast_warning)");
            self.send_wakeup();
        }
    }

    /// Show a configured toast
    #[deprecated(note = "Use cx.toast(...) instead")]
    pub fn show_toast(&self, toast: Toast) {
        if let Ok(mut inner) = self.inner.write() {
            inner.pending_toasts.push(toast);
            log::debug!("AppContext: sending wakeup (show_toast)");
            self.send_wakeup();
        }
    }

    /// Set the current input text (called by runtime for input events)
    pub fn set_input_text(&self, text: String) {
        if let Ok(mut inner) = self.inner.write() {
            inner.input_text = Some(text);
        }
    }

    /// Get the current input text
    pub fn input_text(&self) -> Option<String> {
        self.inner
            .read()
            .ok()
            .and_then(|inner| inner.input_text.clone())
    }

    /// Clear the input text
    pub fn clear_input_text(&self) {
        if let Ok(mut inner) = self.inner.write() {
            inner.input_text = None;
        }
    }

    // -------------------------------------------------------------------------
    // Widget event queue
    // -------------------------------------------------------------------------

    /// Push a widget event to the queue.
    ///
    /// Components call this to signal that an event occurred. The event loop
    /// will drain the queue and dispatch appropriate handlers.
    pub fn push_event(&self, event: WidgetEvent) {
        if let Ok(mut inner) = self.inner.write() {
            inner.pending_events.push(event);
        }
    }

    /// Drain all pending widget events.
    ///
    /// Returns the events and clears the queue. Called by the event loop.
    pub(crate) fn drain_events(&self) -> Vec<WidgetEvent> {
        self.inner
            .write()
            .ok()
            .map(|mut inner| std::mem::take(&mut inner.pending_events))
            .unwrap_or_default()
    }

    // -------------------------------------------------------------------------
    // Unified widget event data accessors
    // -------------------------------------------------------------------------

    /// Set activated item (works for list/tree/table).
    ///
    /// For lists, also provide the index for index-based access.
    pub fn set_activated(&self, id: impl Into<String>, index: Option<usize>) {
        if let Ok(mut inner) = self.inner.write() {
            inner.activated_id = Some(id.into());
            inner.activated_index = index;
        }
    }

    /// Get the activated item ID.
    pub fn activated_id(&self) -> Option<String> {
        self.inner
            .read()
            .ok()
            .and_then(|inner| inner.activated_id.clone())
    }

    /// Get the activated item index (for list).
    pub fn activated_index(&self) -> Option<usize> {
        self.inner
            .read()
            .ok()
            .and_then(|inner| inner.activated_index)
    }

    /// Clear activated item data.
    pub fn clear_activated(&self) {
        if let Ok(mut inner) = self.inner.write() {
            inner.activated_id = None;
            inner.activated_index = None;
        }
    }

    /// Set selected item IDs (works for list/tree/table).
    pub fn set_selected(&self, ids: Vec<String>) {
        if let Ok(mut inner) = self.inner.write() {
            inner.selected_ids = Some(ids);
        }
    }

    /// Get the selected item IDs.
    pub fn selected_ids(&self) -> Option<Vec<String>> {
        self.inner
            .read()
            .ok()
            .and_then(|inner| inner.selected_ids.clone())
    }

    /// Clear selected item data.
    pub fn clear_selected(&self) {
        if let Ok(mut inner) = self.inner.write() {
            inner.selected_ids = None;
        }
    }

    /// Set cursor position (works for list/tree/table).
    ///
    /// For lists, also provide the index for index-based access.
    pub fn set_cursor(&self, id: impl Into<String>, index: Option<usize>) {
        if let Ok(mut inner) = self.inner.write() {
            inner.cursor_id = Some(id.into());
            inner.cursor_index = index;
        }
    }

    /// Get the cursor item ID.
    pub fn cursor_id(&self) -> Option<String> {
        self.inner
            .read()
            .ok()
            .and_then(|inner| inner.cursor_id.clone())
    }

    /// Get the cursor index (for list).
    pub fn cursor_index(&self) -> Option<usize> {
        self.inner.read().ok().and_then(|inner| inner.cursor_index)
    }

    /// Clear cursor data.
    pub fn clear_cursor(&self) {
        if let Ok(mut inner) = self.inner.write() {
            inner.cursor_id = None;
            inner.cursor_index = None;
        }
    }

    /// Set expanded node ID (for tree).
    pub fn set_expanded(&self, id: impl Into<String>) {
        if let Ok(mut inner) = self.inner.write() {
            inner.expanded_id = Some(id.into());
        }
    }

    /// Get the expanded node ID.
    pub fn expanded_id(&self) -> Option<String> {
        self.inner
            .read()
            .ok()
            .and_then(|inner| inner.expanded_id.clone())
    }

    /// Clear expanded node data.
    pub fn clear_expanded(&self) {
        if let Ok(mut inner) = self.inner.write() {
            inner.expanded_id = None;
        }
    }

    /// Set collapsed node ID (for tree).
    pub fn set_collapsed(&self, id: impl Into<String>) {
        if let Ok(mut inner) = self.inner.write() {
            inner.collapsed_id = Some(id.into());
        }
    }

    /// Get the collapsed node ID.
    pub fn collapsed_id(&self) -> Option<String> {
        self.inner
            .read()
            .ok()
            .and_then(|inner| inner.collapsed_id.clone())
    }

    /// Clear collapsed node data.
    pub fn clear_collapsed(&self) {
        if let Ok(mut inner) = self.inner.write() {
            inner.collapsed_id = None;
        }
    }

    /// Set sorted column info (for table).
    pub fn set_sorted(&self, column: usize, ascending: bool) {
        if let Ok(mut inner) = self.inner.write() {
            inner.sorted_column = Some((column, ascending));
        }
    }

    /// Get the sorted column info (column index, ascending).
    pub fn sorted_column(&self) -> Option<(usize, bool)> {
        self.inner.read().ok().and_then(|inner| inner.sorted_column)
    }

    /// Clear sorted column data.
    pub fn clear_sorted(&self) {
        if let Ok(mut inner) = self.inner.write() {
            inner.sorted_column = None;
        }
    }

    /// Publish an event to all non-sleeping app instances.
    ///
    /// Events are delivered asynchronously to all instances that have
    /// an `#[event_handler]` for the event type. This is fire-and-forget;
    /// handlers run concurrently and this method returns immediately.
    ///
    /// Sleeping instances do not receive events.
    ///
    /// # Example
    ///
    /// ```ignore
    /// #[derive(Event, Clone)]
    /// struct UserLoggedIn { user_id: u64 }
    ///
    /// // Publisher
    /// cx.publish(UserLoggedIn { user_id: 123 });
    ///
    /// // Subscriber (in another app)
    /// #[event_handler]
    /// async fn on_login(&self, event: UserLoggedIn, cx: &AppContext) {
    ///     cx.toast(format!("User {} logged in", event.user_id));
    /// }
    /// ```
    pub fn publish<E: crate::event::Event>(&self, event: E) {
        if let Ok(mut inner) = self.inner.write() {
            inner.instance_commands.push(InstanceCommand::PublishEvent {
                event: CloneableEvent::new(event),
            });
            log::debug!("AppContext: sending wakeup (publish_event)");
            self.send_wakeup();
        }
    }

    /// Send a request to the first non-sleeping instance of an app type.
    ///
    /// Returns the response from the handler, or an error if:
    /// - No instance of the app type is running (`NoInstance`)
    /// - The target has no handler for this request type (`NoHandler`)
    /// - The handler panicked (`HandlerPanicked`)
    ///
    /// # Example
    ///
    /// ```ignore
    /// #[derive(Request)]
    /// #[response(bool)]
    /// struct IsPaused;
    ///
    /// // Requester
    /// let paused = cx.request::<QueueApp>(IsPaused).await?;
    ///
    /// // Responder (in QueueApp)
    /// #[request_handler]
    /// async fn is_paused(&self, _req: IsPaused, _cx: &AppContext) -> bool {
    ///     self.paused.get()
    /// }
    /// ```
    pub async fn request<A: App, R: crate::request::Request>(
        &self,
        request: R,
    ) -> Result<R::Response, RequestError> {
        let (tx, rx) = oneshot::channel();

        if let Ok(mut inner) = self.inner.write() {
            inner.instance_commands.push(InstanceCommand::SendRequest {
                target: RequestTarget::AppType(TypeId::of::<A>()),
                request: Box::new(request),
                request_type: TypeId::of::<R>(),
                response_tx: tx,
            });
            log::debug!("AppContext: sending wakeup (request)");
            self.send_wakeup();
        }

        let response = rx.await.map_err(|_| RequestError::HandlerPanicked)??;
        // Downcast from Box<dyn Any + Send + Sync> to Box<R::Response>
        let response: Box<R::Response> = response
            .downcast()
            .map_err(|_| RequestError::HandlerPanicked)?;
        Ok(*response)
    }

    /// Send a request to a specific instance by ID.
    ///
    /// Returns the response from the handler, or an error if:
    /// - The instance does not exist (`InstanceNotFound`)
    /// - The instance is sleeping (`InstanceSleeping`)
    /// - The target has no handler for this request type (`NoHandler`)
    /// - The handler panicked (`HandlerPanicked`)
    ///
    /// # Example
    ///
    /// ```ignore
    /// let paused = cx.request_to::<IsPaused>(queue_id, IsPaused).await?;
    /// ```
    pub async fn request_to<R: crate::request::Request>(
        &self,
        instance_id: InstanceId,
        request: R,
    ) -> Result<R::Response, RequestError> {
        let (tx, rx) = oneshot::channel();

        if let Ok(mut inner) = self.inner.write() {
            inner.instance_commands.push(InstanceCommand::SendRequest {
                target: RequestTarget::Instance(instance_id),
                request: Box::new(request),
                request_type: TypeId::of::<R>(),
                response_tx: tx,
            });
            log::debug!("AppContext: sending wakeup (request_to)");
            self.send_wakeup();
        }

        let response = rx.await.map_err(|_| RequestError::HandlerPanicked)??;
        // Downcast from Box<dyn Any + Send + Sync> to Box<R::Response>
        let response: Box<R::Response> = response
            .downcast()
            .map_err(|_| RequestError::HandlerPanicked)?;
        Ok(*response)
    }

    /// Set the current theme
    ///
    /// The theme change will take effect on the next render.
    pub fn set_theme<T: Theme>(&self, theme: T) {
        if let Ok(mut inner) = self.inner.write() {
            inner.theme_request = Some(Arc::new(theme));
        }
    }

    /// Open a modal and wait for it to return a result.
    ///
    /// The modal will be displayed on top of the current page and will
    /// capture all input until it is closed.
    ///
    /// # Example
    ///
    /// ```ignore
    /// #[handler]
    /// async fn confirm_delete(&self, cx: &AppContext) {
    ///     let confirmed = cx.modal(ConfirmModal {
    ///         message: "Delete this item?".into(),
    ///     }).await;
    ///     
    ///     if confirmed {
    ///         self.delete_item();
    ///     }
    /// }
    /// ```
    pub async fn modal<M: Modal>(&self, modal: M) -> M::Result {
        let (tx, rx) = oneshot::channel();

        // Create the modal context with the result sender
        let mx = ModalContext::new(tx);

        // Create the type-erased entry
        let entry = ModalEntry::new(modal, mx);

        // Request the runtime to push this modal
        if let Ok(mut inner) = self.inner.write() {
            inner.modal_request = Some(Box::new(entry));
            log::debug!("AppContext: sending wakeup (modal)");
            self.send_wakeup();
        }

        // Wait for the modal to close and return its result
        rx.await.expect("modal closed without sending result")
    }

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
    /// async fn load_data(&self, cx: &AppContext) {
    ///     let data = self.data.clone();
    ///     cx.spawn_task(async move {
    ///         data.set_loading();
    ///         let result = fetch_data().await;
    ///         data.set_ready(result);
    ///     });
    /// }
    /// ```
    pub fn spawn_task<F>(&self, future: F) -> JoinHandle<Option<F::Output>>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        let cx = self.clone();
        let app_name = self.current_app_name();
        let instance_id = self.instance_id();

        tokio::spawn(async move {
            let result = AssertUnwindSafe(future).catch_unwind().await;

            match result {
                Ok(value) => Some(value),
                Err(panic) => {
                    if let Some(instance_id) = instance_id {
                        let message = extract_panic_message(&panic);
                        cx.report_error(AppError {
                            app_name,
                            instance_id,
                            kind: AppErrorKind::TaskPanic { message },
                        });
                    }
                    None
                }
            }
        })
    }

    // -------------------------------------------------------------------------
    // Instance management
    // -------------------------------------------------------------------------

    /// Spawn a new app instance.
    ///
    /// The instance is added to the registry but not focused.
    /// Use `spawn_and_focus` to spawn and immediately focus.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let id = cx.spawn::<MyApp>(MyApp::new())?;
    /// ```
    pub fn spawn<A: App>(&self, app: A) -> Result<InstanceId, SpawnError> {
        use crate::app::AppInstance;

        let config = A::config();

        // Check max instances using the registry
        if let Some(max) = config.max_instances {
            let current = self.instance_count::<A>();
            if current >= max {
                return Err(SpawnError::MaxInstancesReached {
                    app_name: config.name,
                    max,
                });
            }
        }

        let instance = AppInstance::new(app);
        let id = instance.id();

        // Queue the spawn command
        if let Ok(mut inner) = self.inner.write() {
            inner.instance_commands.push(InstanceCommand::Spawn {
                instance: Box::new(instance),
                focus: false,
            });
            log::debug!("AppContext: sending wakeup (spawn)");
            self.send_wakeup();
        }

        Ok(id)
    }

    /// Spawn a new app instance and immediately focus it.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let id = cx.spawn_and_focus::<MyApp>(MyApp::new())?;
    /// ```
    pub fn spawn_and_focus<A: App>(&self, app: A) -> Result<InstanceId, SpawnError> {
        use crate::app::AppInstance;

        let config = A::config();

        // Check max instances using the registry
        if let Some(max) = config.max_instances {
            let current = self.instance_count::<A>();
            if current >= max {
                return Err(SpawnError::MaxInstancesReached {
                    app_name: config.name,
                    max,
                });
            }
        }

        let instance = AppInstance::new(app);
        let id = instance.id();

        // Queue the spawn command with focus
        if let Ok(mut inner) = self.inner.write() {
            inner.instance_commands.push(InstanceCommand::Spawn {
                instance: Box::new(instance),
                focus: true,
            });
            log::debug!("AppContext: sending wakeup (spawn_and_focus)");
            self.send_wakeup();
        }

        Ok(id)
    }

    /// Close an instance.
    ///
    /// Respects `on_close_request` - if it returns false, the close is cancelled.
    /// Use `force_close` to skip this check.
    ///
    /// Returns immediately; the actual close happens in the event loop.
    pub fn close(&self, id: InstanceId) {
        if let Ok(mut inner) = self.inner.write() {
            inner
                .instance_commands
                .push(InstanceCommand::Close { id, force: false });
            log::debug!("AppContext: sending wakeup (close)");
            self.send_wakeup();
        }
    }

    /// Force close an instance.
    ///
    /// Skips `on_close_request` but respects the `persistent` flag.
    /// Persistent apps cannot be force-closed.
    pub fn force_close(&self, id: InstanceId) {
        if let Ok(mut inner) = self.inner.write() {
            inner
                .instance_commands
                .push(InstanceCommand::Close { id, force: true });
            log::debug!("AppContext: sending wakeup (force_close)");
            self.send_wakeup();
        }
    }

    /// Focus an instance.
    ///
    /// Makes the instance the foreground app, receiving input and rendering.
    /// The previously focused instance receives `on_background`.
    pub fn focus_instance(&self, id: InstanceId) {
        log::debug!("focus_instance called with id: {:?}", id);
        if let Ok(mut inner) = self.inner.write() {
            inner.instance_commands.push(InstanceCommand::Focus { id });
            log::debug!("AppContext: sending wakeup (focus_instance)");
            self.send_wakeup();
        }
    }

    /// Get the current instance ID.
    ///
    /// Returns the ID of the instance that owns this context.
    pub fn instance_id(&self) -> Option<InstanceId> {
        self.inner
            .read()
            .ok()
            .and_then(|inner| inner.current_instance_id)
    }

    /// Get the current app's name.
    ///
    /// Returns the name from the app's config, or "unknown" if not available.
    /// Used internally for error reporting.
    pub(crate) fn current_app_name(&self) -> &'static str {
        let instance_id = match self.instance_id() {
            Some(id) => id,
            None => return "unknown",
        };

        self.registry
            .as_ref()
            .and_then(|r| r.read().ok())
            .and_then(|reg| reg.get(instance_id).map(|i| i.config().name))
            .unwrap_or("unknown")
    }

    // -------------------------------------------------------------------------
    // Instance discovery
    // -------------------------------------------------------------------------

    /// List all running instances.
    pub fn instances(&self) -> Vec<InstanceInfo> {
        self.registry
            .as_ref()
            .and_then(|r| r.read().ok())
            .map(|reg| reg.instances())
            .unwrap_or_default()
    }

    /// List instances of a specific app type.
    pub fn instances_of<A: App>(&self) -> Vec<InstanceInfo> {
        self.registry
            .as_ref()
            .and_then(|r| r.read().ok())
            .map(|reg| reg.instances_of::<A>())
            .unwrap_or_default()
    }

    /// Find the first instance of a specific app type.
    ///
    /// Useful for singleton apps to check if an instance already exists.
    pub fn instance_of<A: App>(&self) -> Option<InstanceId> {
        self.registry
            .as_ref()
            .and_then(|r| r.read().ok())
            .and_then(|reg| reg.instance_of::<A>())
    }

    /// Get the number of instances of a specific app type.
    pub fn instance_count<A: App>(&self) -> usize {
        self.registry
            .as_ref()
            .and_then(|r| r.read().ok())
            .map(|reg| reg.instance_count::<A>())
            .unwrap_or(0)
    }

    // -------------------------------------------------------------------------
    // Keybind methods
    // -------------------------------------------------------------------------

    /// Get display info for all keybinds
    pub fn keybinds(&self) -> Vec<KeybindInfo> {
        self.keybinds
            .read()
            .map(|kb| kb.infos())
            .unwrap_or_default()
    }

    /// Get display info for currently active keybinds
    ///
    /// Note: This requires knowing the current page, which is app-specific.
    /// For now, this returns all keybinds. Use the app's `current_page()`
    /// to filter if needed.
    pub fn all_keybinds(&self) -> Vec<KeybindInfo> {
        self.keybinds()
    }

    /// Override a keybind's key combination
    ///
    /// # Example
    /// ```ignore
    /// cx.override_keybind("my_app.save", "ctrl+shift+s")?;
    /// ```
    pub fn override_keybind(&self, id: &str, keys: &str) -> Result<(), KeybindError> {
        let mut keybinds = self
            .keybinds
            .write()
            .map_err(|_| KeybindError::ParseError("Failed to acquire keybinds lock".to_string()))?;
        keybinds.override_keybind(id, keys)
    }

    /// Disable a keybind
    pub fn disable_keybind(&self, id: &str) -> Result<(), KeybindError> {
        let mut keybinds = self
            .keybinds
            .write()
            .map_err(|_| KeybindError::ParseError("Failed to acquire keybinds lock".to_string()))?;
        keybinds.disable_keybind(id)
    }

    /// Reset a keybind to its default key combination
    pub fn reset_keybind(&self, id: &str) -> Result<(), KeybindError> {
        let mut keybinds = self
            .keybinds
            .write()
            .map_err(|_| KeybindError::ParseError("Failed to acquire keybinds lock".to_string()))?;
        keybinds.reset_keybind(id)
    }

    /// Reset all keybinds to their defaults
    pub fn reset_all_keybinds(&self) {
        if let Ok(mut keybinds) = self.keybinds.write() {
            keybinds.reset_all();
        }
    }

    // -------------------------------------------------------------------------
    // Internal methods for runtime use
    // -------------------------------------------------------------------------

    /// Check if exit was requested (runtime use)
    pub(crate) fn is_exit_requested(&self) -> bool {
        self.inner
            .read()
            .map(|inner| inner.exit_requested)
            .unwrap_or(false)
    }

    /// Take the focus request (runtime use)
    pub(crate) fn take_focus_request(&self) -> Option<FocusId> {
        self.inner
            .write()
            .ok()
            .and_then(|mut inner| inner.focus_request.take())
    }

    /// Take pending toasts (runtime use)
    pub(crate) fn take_toasts(&self) -> Vec<Toast> {
        self.inner
            .write()
            .ok()
            .map(|mut inner| std::mem::take(&mut inner.pending_toasts))
            .unwrap_or_default()
    }

    /// Take the theme change request (runtime use)
    pub(crate) fn take_theme_request(&self) -> Option<Arc<dyn Theme>> {
        self.inner
            .write()
            .ok()
            .and_then(|mut inner| inner.theme_request.take())
    }

    /// Take the modal request (runtime use)
    pub(crate) fn take_modal_request(&self) -> Option<Box<dyn ModalDyn>> {
        self.inner
            .write()
            .ok()
            .and_then(|mut inner| inner.modal_request.take())
    }

    /// Take pending instance commands (runtime use)
    pub(crate) fn take_instance_commands(&self) -> Vec<InstanceCommand> {
        self.inner
            .write()
            .ok()
            .map(|mut inner| std::mem::take(&mut inner.instance_commands))
            .unwrap_or_default()
    }

    /// Set the instance registry (runtime use)
    pub(crate) fn set_registry(&mut self, registry: Arc<RwLock<InstanceRegistry>>) {
        self.registry = Some(registry);
    }

    /// Set the current instance ID (runtime use)
    pub(crate) fn set_instance_id(&self, id: InstanceId) {
        if let Ok(mut inner) = self.inner.write() {
            inner.current_instance_id = Some(id);
        }
    }
}

impl Default for AppContext {
    fn default() -> Self {
        use std::collections::HashMap;
        Self {
            inner: Arc::new(RwLock::new(AppContextInner {
                exit_requested: false,
                focus_request: None,
                pending_toasts: Vec::new(),
                input_text: None,
                theme_request: None,
                modal_request: None,
                instance_commands: Vec::new(),
                current_instance_id: None,
                pending_events: Vec::new(),
                activated_id: None,
                activated_index: None,
                selected_ids: None,
                cursor_id: None,
                cursor_index: None,
                expanded_id: None,
                collapsed_id: None,
                sorted_column: None,
            })),
            keybinds: Arc::new(RwLock::new(Keybinds::new())),
            registry: None,
            data: Arc::new(HashMap::new()),
            wakeup_sender: None,
            error_sender: None,
        }
    }
}

/// Context passed to page render functions
pub struct ViewContext<'a> {
    /// Whether reduce_motion is enabled
    reduce_motion: bool,
    /// Reference to app context for focus etc
    _app_cx: &'a AppContext,
}

impl<'a> ViewContext<'a> {
    /// Create a new page context
    pub fn new(app_cx: &'a AppContext) -> Self {
        Self {
            reduce_motion: false,
            _app_cx: app_cx,
        }
    }

    /// Check if reduce_motion is enabled
    pub fn reduce_motion(&self) -> bool {
        self.reduce_motion
    }
}
