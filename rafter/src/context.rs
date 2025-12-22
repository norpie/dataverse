use std::future::Future;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use crate::widgets::events::WidgetEvent;
use crate::input::focus::FocusId;
use crate::input::keybinds::{KeybindError, KeybindInfo, Keybinds};
use crate::layers::modal::{Modal, ModalContext, ModalDyn, ModalEntry};
use crate::styling::theme::Theme;

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
    /// Message to display
    pub message: String,
    /// Toast level (affects styling)
    pub level: ToastLevel,
    /// How long to show the toast
    pub duration: Duration,
}

impl Toast {
    /// Create a simple info toast
    pub fn info(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            level: ToastLevel::Info,
            duration: Duration::from_secs(3),
        }
    }

    /// Create an error toast
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            level: ToastLevel::Error,
            duration: Duration::from_secs(5),
        }
    }

    /// Create a success toast
    pub fn success(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            level: ToastLevel::Success,
            duration: Duration::from_secs(3),
        }
    }

    /// Create a warning toast
    pub fn warning(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            level: ToastLevel::Warning,
            duration: Duration::from_secs(4),
        }
    }
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
#[derive(Clone)]
pub struct AppContext {
    inner: Arc<RwLock<AppContextInner>>,
    /// Shared keybinds (can be modified at runtime)
    keybinds: Arc<RwLock<Keybinds>>,
}

impl AppContext {
    /// Create a new app context with shared keybinds
    pub fn new(keybinds: Arc<RwLock<Keybinds>>) -> Self {
        Self {
            inner: Arc::new(RwLock::new(AppContextInner {
                exit_requested: false,
                focus_request: None,
                pending_toasts: Vec::new(),
                input_text: None,
                theme_request: None,
                modal_request: None,
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
        }
    }

    /// Request to exit the current app
    pub fn exit(&self) {
        if let Ok(mut inner) = self.inner.write() {
            inner.exit_requested = true;
        }
    }

    /// Set focus to a specific element by ID
    pub fn focus(&self, id: impl Into<FocusId>) {
        if let Ok(mut inner) = self.inner.write() {
            inner.focus_request = Some(id.into());
        }
    }

    /// Show a toast notification
    pub fn toast(&self, message: impl Into<String>) {
        if let Ok(mut inner) = self.inner.write() {
            inner.pending_toasts.push(Toast::info(message));
        }
    }

    /// Show an error toast notification
    pub fn toast_error(&self, message: impl Into<String>) {
        if let Ok(mut inner) = self.inner.write() {
            inner.pending_toasts.push(Toast::error(message));
        }
    }

    /// Show a success toast notification
    pub fn toast_success(&self, message: impl Into<String>) {
        if let Ok(mut inner) = self.inner.write() {
            inner.pending_toasts.push(Toast::success(message));
        }
    }

    /// Show a warning toast notification
    pub fn toast_warning(&self, message: impl Into<String>) {
        if let Ok(mut inner) = self.inner.write() {
            inner.pending_toasts.push(Toast::warning(message));
        }
    }

    /// Show a configured toast
    pub fn show_toast(&self, toast: Toast) {
        if let Ok(mut inner) = self.inner.write() {
            inner.pending_toasts.push(toast);
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

    /// Publish an event to the event bus
    pub fn publish<E: 'static + Send>(&self, _event: E) {
        // TODO: implement pub/sub
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
        }

        // Wait for the modal to close and return its result
        rx.await.expect("modal closed without sending result")
    }

    /// Spawn an async task.
    ///
    /// The spawned task runs independently and can use cloned state.
    ///
    /// # Example
    ///
    /// ```ignore
    /// #[handler]
    /// async fn load_data(&self, cx: &AppContext) {
    ///     let data = self.data.clone();
    ///     cx.spawn(async move {
    ///         data.set_loading();
    ///         let result = fetch_data().await;
    ///         data.set_ready(result);
    ///     });
    /// }
    /// ```
    pub fn spawn<F>(&self, future: F) -> JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        tokio::spawn(future)
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
}

impl Default for AppContext {
    fn default() -> Self {
        Self::new(Arc::new(RwLock::new(Keybinds::new())))
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
