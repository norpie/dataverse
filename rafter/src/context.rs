use std::future::Future;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use crate::focus::FocusId;
use crate::keybinds::{KeybindError, KeybindInfo, Keybinds};
use crate::modal::{Modal, ModalContext, ModalDyn, ModalEntry};
use crate::theme::Theme;

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
    /// Activated list item index (for list on_activate handlers)
    list_activated_index: Option<usize>,
    /// Selected list indices (for list on_selection_change handlers)
    list_selected_indices: Option<Vec<usize>>,
    /// Cursor position (for list on_cursor_move handlers)
    list_cursor: Option<usize>,
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
                list_activated_index: None,
                list_selected_indices: None,
                list_cursor: None,
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
    // List event data (set by runtime for list handlers)
    // -------------------------------------------------------------------------

    /// Set the activated list item index (called by runtime)
    pub fn set_list_activated_index(&self, index: usize) {
        if let Ok(mut inner) = self.inner.write() {
            inner.list_activated_index = Some(index);
        }
    }

    /// Get the activated list item index
    pub fn list_activated_index(&self) -> Option<usize> {
        self.inner
            .read()
            .ok()
            .and_then(|inner| inner.list_activated_index)
    }

    /// Clear the activated list item index
    pub fn clear_list_activated_index(&self) {
        if let Ok(mut inner) = self.inner.write() {
            inner.list_activated_index = None;
        }
    }

    /// Set the selected list indices (called by runtime)
    pub fn set_list_selected_indices(&self, indices: Vec<usize>) {
        if let Ok(mut inner) = self.inner.write() {
            inner.list_selected_indices = Some(indices);
        }
    }

    /// Get the selected list indices
    pub fn list_selected_indices(&self) -> Option<Vec<usize>> {
        self.inner
            .read()
            .ok()
            .and_then(|inner| inner.list_selected_indices.clone())
    }

    /// Clear the selected list indices
    pub fn clear_list_selected_indices(&self) {
        if let Ok(mut inner) = self.inner.write() {
            inner.list_selected_indices = None;
        }
    }

    /// Set the list cursor position (called by runtime)
    pub fn set_list_cursor(&self, cursor: usize) {
        if let Ok(mut inner) = self.inner.write() {
            inner.list_cursor = Some(cursor);
        }
    }

    /// Get the list cursor position
    pub fn list_cursor(&self) -> Option<usize> {
        self.inner.read().ok().and_then(|inner| inner.list_cursor)
    }

    /// Clear the list cursor position
    pub fn clear_list_cursor(&self) {
        if let Ok(mut inner) = self.inner.write() {
            inner.list_cursor = None;
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
    /// The modal will be displayed on top of the current view and will
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
    /// Note: This requires knowing the current view, which is app-specific.
    /// For now, this returns all keybinds. Use the app's `current_view()` 
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
        let mut keybinds = self.keybinds.write().map_err(|_| {
            KeybindError::ParseError("Failed to acquire keybinds lock".to_string())
        })?;
        keybinds.override_keybind(id, keys)
    }

    /// Disable a keybind
    pub fn disable_keybind(&self, id: &str) -> Result<(), KeybindError> {
        let mut keybinds = self.keybinds.write().map_err(|_| {
            KeybindError::ParseError("Failed to acquire keybinds lock".to_string())
        })?;
        keybinds.disable_keybind(id)
    }

    /// Reset a keybind to its default key combination
    pub fn reset_keybind(&self, id: &str) -> Result<(), KeybindError> {
        let mut keybinds = self.keybinds.write().map_err(|_| {
            KeybindError::ParseError("Failed to acquire keybinds lock".to_string())
        })?;
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

/// Context passed to view render functions
pub struct ViewContext<'a> {
    /// Whether reduce_motion is enabled
    reduce_motion: bool,
    /// Reference to app context for focus etc
    _app_cx: &'a AppContext,
}

impl<'a> ViewContext<'a> {
    /// Create a new view context
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
