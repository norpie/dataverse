use std::any::Any;
use std::future::Future;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use tokio::task::JoinHandle;

use crate::focus::FocusId;
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
    /// Request to navigate to a different view
    navigate_to: Option<Box<dyn Any + Send + Sync>>,
    /// Request to focus a specific element
    focus_request: Option<FocusId>,
    /// Pending toasts to show
    pending_toasts: Vec<Toast>,
    /// Text input from the current input event
    input_text: Option<String>,
    /// Request to change theme
    theme_request: Option<Arc<dyn Theme>>,
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
}

impl AppContext {
    /// Create a new app context
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(AppContextInner {
                exit_requested: false,
                navigate_to: None,
                focus_request: None,
                pending_toasts: Vec::new(),
                input_text: None,
                theme_request: None,
            })),
        }
    }

    /// Request to exit the current app
    pub fn exit(&self) {
        if let Ok(mut inner) = self.inner.write() {
            inner.exit_requested = true;
        }
    }

    /// Navigate to a different view
    pub fn navigate<V: 'static + Send + Sync>(&self, view: V) {
        if let Ok(mut inner) = self.inner.write() {
            inner.navigate_to = Some(Box::new(view));
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
    // Internal methods for runtime use
    // -------------------------------------------------------------------------

    /// Check if exit was requested (runtime use)
    pub(crate) fn is_exit_requested(&self) -> bool {
        self.inner
            .read()
            .map(|inner| inner.exit_requested)
            .unwrap_or(false)
    }

    /// Take the navigation request (runtime use)
    pub(crate) fn take_navigation(&self) -> Option<Box<dyn Any + Send + Sync>> {
        self.inner.write().ok().and_then(|mut inner| inner.navigate_to.take())
    }

    /// Take the focus request (runtime use)
    pub(crate) fn take_focus_request(&self) -> Option<FocusId> {
        self.inner.write().ok().and_then(|mut inner| inner.focus_request.take())
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
        self.inner.write().ok().and_then(|mut inner| inner.theme_request.take())
    }
}

impl Default for AppContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Context passed to modal handlers (will be expanded for modal system)
pub struct ModalContext {
    /// The result to emit when modal closes
    result: Option<Box<dyn Any + Send>>,
    /// Whether modal should close
    should_close: bool,
}

impl ModalContext {
    /// Create a new modal context
    pub fn new() -> Self {
        Self {
            result: None,
            should_close: false,
        }
    }

    /// Emit a result value
    pub fn emit<T: 'static + Send>(&mut self, value: T) {
        self.result = Some(Box::new(value));
    }

    /// Close the modal
    pub fn close(&mut self) {
        self.should_close = true;
    }

    /// Check if modal should close
    pub fn should_close(&self) -> bool {
        self.should_close
    }

    /// Take the result
    pub fn take_result(&mut self) -> Option<Box<dyn Any + Send>> {
        self.result.take()
    }
}

impl Default for ModalContext {
    fn default() -> Self {
        Self::new()
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
