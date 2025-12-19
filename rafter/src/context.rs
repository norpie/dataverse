/// Context passed to app handlers, providing access to framework functionality.
pub struct AppContext {
    /// Request to exit the app
    exit_requested: bool,
    /// Request to navigate to a different view
    navigate_to: Option<Box<dyn std::any::Any + Send>>,
}

impl AppContext {
    /// Create a new app context
    pub fn new() -> Self {
        Self {
            exit_requested: false,
            navigate_to: None,
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
    pub fn focus(&mut self, _id: &str) {
        // TODO: implement focus system
    }

    /// Show a toast notification
    pub fn toast(&mut self, _message: impl Into<String>) {
        // TODO: implement toast system
    }

    /// Publish an event to the event bus
    pub fn publish<E: 'static + Send>(&mut self, _event: E) {
        // TODO: implement pub/sub
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
