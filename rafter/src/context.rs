use std::future::Future;
use std::time::Duration;

use tokio::task::JoinHandle;

use crate::focus::FocusId;

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

/// Context passed to app handlers, providing access to framework functionality.
pub struct AppContext {
    /// Request to exit the app
    exit_requested: bool,
    /// Request to navigate to a different view
    navigate_to: Option<Box<dyn std::any::Any + Send>>,
    /// Request to focus a specific element
    focus_request: Option<FocusId>,
    /// Pending toasts to show
    pending_toasts: Vec<Toast>,
    /// Text input from the current input event
    input_text: Option<String>,
}

impl AppContext {
    /// Create a new app context
    pub fn new() -> Self {
        Self {
            exit_requested: false,
            navigate_to: None,
            focus_request: None,
            pending_toasts: Vec::new(),
            input_text: None,
        }
    }

    /// Request to exit the current app
    pub fn exit(&mut self) {
        self.exit_requested = true;
    }

    /// Check if exit was requested
    pub fn is_exit_requested(&self) -> bool {
        self.exit_requested
    }

    /// Navigate to a different view
    pub fn navigate<V: 'static + Send>(&mut self, view: V) {
        self.navigate_to = Some(Box::new(view));
    }

    /// Take the navigation request
    pub fn take_navigation(&mut self) -> Option<Box<dyn std::any::Any + Send>> {
        self.navigate_to.take()
    }

    /// Set focus to a specific element by ID
    pub fn focus(&mut self, id: impl Into<FocusId>) {
        self.focus_request = Some(id.into());
    }

    /// Take the focus request
    pub fn take_focus_request(&mut self) -> Option<FocusId> {
        self.focus_request.take()
    }

    /// Show a toast notification
    pub fn toast(&mut self, message: impl Into<String>) {
        self.pending_toasts.push(Toast::info(message));
    }

    /// Show a configured toast
    pub fn show_toast(&mut self, toast: Toast) {
        self.pending_toasts.push(toast);
    }

    /// Take pending toasts
    pub fn take_toasts(&mut self) -> Vec<Toast> {
        std::mem::take(&mut self.pending_toasts)
    }

    /// Set the current input text (called by runtime for input events)
    pub fn set_input_text(&mut self, text: String) {
        self.input_text = Some(text);
    }

    /// Get the current input text
    pub fn input_text(&self) -> Option<&str> {
        self.input_text.as_deref()
    }

    /// Clear the input text
    pub fn clear_input_text(&mut self) {
        self.input_text = None;
    }

    /// Publish an event to the event bus
    pub fn publish<E: 'static + Send>(&mut self, _event: E) {
        // TODO: implement pub/sub
    }

    /// Spawn an async task.
    ///
    /// Use this for fire-and-forget async work. The spawned task can mutate
    /// `AsyncResource<T>` and `AsyncState<T>` fields (after cloning them).
    ///
    /// # Example
    ///
    /// ```ignore
    /// #[handler]
    /// fn load_data(&mut self, cx: &mut AppContext) {
    ///     let data = self.data.clone(); // AsyncResource<T>
    ///     cx.spawn(async move {
    ///         data.set(Resource::Loading);
    ///         let result = fetch_data().await;
    ///         data.set(Resource::Ready(result));
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
}

impl Default for AppContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Context passed to modal handlers
pub struct ModalContext {
    /// The result to emit when modal closes
    result: Option<Box<dyn std::any::Any + Send>>,
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
    pub fn take_result(&mut self) -> Option<Box<dyn std::any::Any + Send>> {
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
