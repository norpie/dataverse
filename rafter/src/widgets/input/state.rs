use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};

use crate::validation::ErrorDisplay;

/// Unique identifier for an Input widget instance
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InputId(usize);

impl InputId {
    fn new() -> Self {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        Self(COUNTER.fetch_add(1, Ordering::SeqCst))
    }
}

impl std::fmt::Display for InputId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "__input_{}", self.0)
    }
}

/// Internal state for an Input widget
#[derive(Debug, Default)]
struct InputInner {
    /// Current text value
    value: String,
    /// Placeholder text
    placeholder: String,
    /// Cursor position (byte offset)
    cursor: usize,
    /// Validation error message (if any)
    error: Option<String>,
    /// How to display validation errors
    error_display: ErrorDisplay,
}

/// A text input widget with reactive state.
///
/// `Input` is a self-contained widget that manages its own text value,
/// cursor position, and focus state. It provides imperative methods for
/// reading and modifying the input programmatically.
///
/// # Example
///
/// ```ignore
/// #[app]
/// struct MyApp {
///     name_input: Input,  // Not wrapped in State<T>
/// }
///
/// #[app_impl]
/// impl MyApp {
///     fn page(&self) -> Node {
///         page! {
///             column {
///                 input(bind: self.name_input, placeholder: "Enter name")
///                 button(label: "Submit", on_click: Self::submit)
///             }
///         }
///     }
///
///     #[handler]
///     async fn submit(&self, cx: &AppContext) {
///         let name = self.name_input.value();
///         // ... use name
///         self.name_input.clear();
///     }
/// }
/// ```
#[derive(Debug)]
pub struct Input {
    /// Unique identifier for this input instance
    id: InputId,
    /// Internal state
    inner: Arc<RwLock<InputInner>>,
    /// Dirty flag for re-render
    dirty: Arc<AtomicBool>,
    /// Focus request flag (checked by runtime)
    focus_requested: Arc<AtomicBool>,
}

impl Input {
    /// Create a new empty input
    pub fn new() -> Self {
        Self {
            id: InputId::new(),
            inner: Arc::new(RwLock::new(InputInner::default())),
            dirty: Arc::new(AtomicBool::new(false)),
            focus_requested: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Create an input with an initial value
    pub fn with_value(value: impl Into<String>) -> Self {
        let value = value.into();
        let cursor = value.len();
        Self {
            id: InputId::new(),
            inner: Arc::new(RwLock::new(InputInner {
                value,
                cursor,
                placeholder: String::new(),
                error: None,
                error_display: ErrorDisplay::default(),
            })),
            dirty: Arc::new(AtomicBool::new(false)),
            focus_requested: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Create an input with a placeholder
    pub fn with_placeholder(placeholder: impl Into<String>) -> Self {
        Self {
            id: InputId::new(),
            inner: Arc::new(RwLock::new(InputInner {
                placeholder: placeholder.into(),
                ..Default::default()
            })),
            dirty: Arc::new(AtomicBool::new(false)),
            focus_requested: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Get the unique ID for this input
    pub fn id(&self) -> InputId {
        self.id
    }

    /// Get the ID as a string (for node binding)
    pub fn id_string(&self) -> String {
        self.id.to_string()
    }

    // -------------------------------------------------------------------------
    // Read methods
    // -------------------------------------------------------------------------

    /// Get the current text value
    pub fn value(&self) -> String {
        self.inner
            .read()
            .map(|guard| guard.value.clone())
            .unwrap_or_default()
    }

    /// Get the placeholder text
    pub fn placeholder(&self) -> String {
        self.inner
            .read()
            .map(|guard| guard.placeholder.clone())
            .unwrap_or_default()
    }

    /// Get the cursor position (byte offset)
    pub fn cursor(&self) -> usize {
        self.inner.read().map(|guard| guard.cursor).unwrap_or(0)
    }

    /// Check if the input is empty
    pub fn is_empty(&self) -> bool {
        self.inner
            .read()
            .map(|guard| guard.value.is_empty())
            .unwrap_or(true)
    }

    /// Get the length of the current value
    pub fn len(&self) -> usize {
        self.inner
            .read()
            .map(|guard| guard.value.len())
            .unwrap_or(0)
    }

    // -------------------------------------------------------------------------
    // Write methods
    // -------------------------------------------------------------------------

    /// Set the text value
    pub fn set_value(&self, value: impl Into<String>) {
        if let Ok(mut guard) = self.inner.write() {
            guard.value = value.into();
            guard.cursor = guard.value.len();
            guard.error = None; // Auto-clear error on value change
            self.dirty.store(true, Ordering::SeqCst);
        }
    }

    /// Clear the input value
    pub fn clear(&self) {
        if let Ok(mut guard) = self.inner.write() {
            guard.value.clear();
            guard.cursor = 0;
            guard.error = None; // Auto-clear error on value change
            self.dirty.store(true, Ordering::SeqCst);
        }
    }

    /// Set the placeholder text
    pub fn set_placeholder(&self, placeholder: impl Into<String>) {
        if let Ok(mut guard) = self.inner.write() {
            guard.placeholder = placeholder.into();
            self.dirty.store(true, Ordering::SeqCst);
        }
    }

    /// Set the cursor position
    pub fn set_cursor(&self, position: usize) {
        if let Ok(mut guard) = self.inner.write() {
            guard.cursor = position.min(guard.value.len());
            self.dirty.store(true, Ordering::SeqCst);
        }
    }

    // -------------------------------------------------------------------------
    // Text manipulation (called by runtime on key events)
    // -------------------------------------------------------------------------

    /// Insert a character at the cursor position
    pub fn insert_char(&self, c: char) {
        if let Ok(mut guard) = self.inner.write() {
            let cursor = guard.cursor;
            guard.value.insert(cursor, c);
            guard.cursor += c.len_utf8();
            guard.error = None; // Auto-clear error on value change
            self.dirty.store(true, Ordering::SeqCst);
        }
    }

    /// Delete the character before the cursor (backspace)
    pub fn delete_char_before(&self) {
        if let Ok(mut guard) = self.inner.write()
            && guard.cursor > 0
        {
            // Find the previous character boundary
            let prev_cursor = guard.value[..guard.cursor]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
            guard.value.remove(prev_cursor);
            guard.cursor = prev_cursor;
            guard.error = None; // Auto-clear error on value change
            self.dirty.store(true, Ordering::SeqCst);
        }
    }

    /// Delete the character at the cursor (delete key)
    pub fn delete_char_at(&self) {
        if let Ok(mut guard) = self.inner.write() {
            let cursor = guard.cursor;
            if cursor < guard.value.len() {
                guard.value.remove(cursor);
                guard.error = None; // Auto-clear error on value change
                self.dirty.store(true, Ordering::SeqCst);
            }
        }
    }

    /// Move cursor left
    pub fn cursor_left(&self) {
        if let Ok(mut guard) = self.inner.write()
            && guard.cursor > 0
        {
            guard.cursor = guard.value[..guard.cursor]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.dirty.store(true, Ordering::SeqCst);
        }
    }

    /// Move cursor right
    pub fn cursor_right(&self) {
        if let Ok(mut guard) = self.inner.write()
            && guard.cursor < guard.value.len()
        {
            guard.cursor = guard.value[guard.cursor..]
                .char_indices()
                .nth(1)
                .map(|(i, _)| guard.cursor + i)
                .unwrap_or(guard.value.len());
            self.dirty.store(true, Ordering::SeqCst);
        }
    }

    /// Move cursor to start
    pub fn cursor_home(&self) {
        if let Ok(mut guard) = self.inner.write()
            && guard.cursor != 0
        {
            guard.cursor = 0;
            self.dirty.store(true, Ordering::SeqCst);
        }
    }

    /// Move cursor to end
    pub fn cursor_end(&self) {
        if let Ok(mut guard) = self.inner.write() {
            let end = guard.value.len();
            if guard.cursor != end {
                guard.cursor = end;
                self.dirty.store(true, Ordering::SeqCst);
            }
        }
    }

    // -------------------------------------------------------------------------
    // Focus control
    // -------------------------------------------------------------------------

    /// Request focus for this input
    pub fn focus(&self) {
        self.focus_requested.store(true, Ordering::SeqCst);
    }

    /// Check and clear the focus request (called by runtime)
    pub fn take_focus_request(&self) -> bool {
        self.focus_requested.swap(false, Ordering::SeqCst)
    }

    // -------------------------------------------------------------------------
    // Dirty tracking
    // -------------------------------------------------------------------------

    /// Check if the input state has changed
    pub fn is_dirty(&self) -> bool {
        self.dirty.load(Ordering::SeqCst)
    }

    /// Clear the dirty flag
    pub fn clear_dirty(&self) {
        self.dirty.store(false, Ordering::SeqCst);
    }

    // -------------------------------------------------------------------------
    // Validation
    // -------------------------------------------------------------------------

    /// Set a validation error message on this input.
    pub fn set_error(&self, msg: impl Into<String>) {
        if let Ok(mut guard) = self.inner.write() {
            guard.error = Some(msg.into());
            self.dirty.store(true, Ordering::SeqCst);
        }
    }

    /// Clear the validation error.
    pub fn clear_error(&self) {
        if let Ok(mut guard) = self.inner.write()
            && guard.error.is_some() {
                guard.error = None;
                self.dirty.store(true, Ordering::SeqCst);
            }
    }

    /// Check if this input has a validation error.
    pub fn has_error(&self) -> bool {
        self.inner
            .read()
            .map(|guard| guard.error.is_some())
            .unwrap_or(false)
    }

    /// Get the current validation error message (if any).
    pub fn error(&self) -> Option<String> {
        self.inner
            .read()
            .map(|guard| guard.error.clone())
            .unwrap_or(None)
    }

    /// Get the error display mode.
    pub fn error_display(&self) -> ErrorDisplay {
        self.inner
            .read()
            .map(|guard| guard.error_display)
            .unwrap_or_default()
    }

    /// Set the error display mode.
    pub fn set_error_display(&self, display: ErrorDisplay) {
        if let Ok(mut guard) = self.inner.write() {
            guard.error_display = display;
            self.dirty.store(true, Ordering::SeqCst);
        }
    }
}

impl Clone for Input {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            inner: Arc::clone(&self.inner),
            dirty: Arc::clone(&self.dirty),
            focus_requested: Arc::clone(&self.focus_requested),
        }
    }
}

impl Default for Input {
    fn default() -> Self {
        Self::new()
    }
}

// -----------------------------------------------------------------------------
// Validatable implementation
// -----------------------------------------------------------------------------

use crate::validation::Validatable;

impl Validatable for Input {
    type Value = String;

    fn validation_value(&self) -> Self::Value {
        self.value()
    }

    fn set_error(&self, msg: impl Into<String>) {
        Input::set_error(self, msg)
    }

    fn clear_error(&self) {
        Input::clear_error(self)
    }

    fn has_error(&self) -> bool {
        Input::has_error(self)
    }

    fn error(&self) -> Option<String> {
        Input::error(self)
    }

    fn widget_id(&self) -> String {
        self.id_string()
    }

    fn error_display(&self) -> ErrorDisplay {
        Input::error_display(self)
    }

    fn set_error_display(&self, display: ErrorDisplay) {
        Input::set_error_display(self, display)
    }
}
