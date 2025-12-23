//! Select widget state.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};

use ratatui::layout::Rect;

use crate::validation::ErrorDisplay;

/// Unique identifier for a Select widget instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SelectId(usize);

impl SelectId {
    fn new() -> Self {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        Self(COUNTER.fetch_add(1, Ordering::SeqCst))
    }
}

impl std::fmt::Display for SelectId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "__select_{}", self.0)
    }
}

/// Internal state for a Select widget.
#[derive(Debug, Default)]
struct SelectInner {
    /// Currently selected index (None if nothing selected)
    selected_index: Option<usize>,
    /// Placeholder text shown when nothing is selected
    placeholder: String,
    /// Validation error message (if any)
    error: Option<String>,
    /// How to display validation errors
    error_display: ErrorDisplay,
    /// Cached anchor rect for overlay positioning
    anchor_rect: Option<Rect>,
    /// Number of options (set during render)
    options_count: usize,
    /// Labels for options (set during render for display)
    option_labels: Vec<String>,
}

/// A dropdown select widget with reactive state.
///
/// `Select` is a self-contained widget that manages its own selection state.
/// When focused, it can be opened to show a dropdown of options. The selected
/// option is displayed when closed.
///
/// # Example
///
/// ```ignore
/// #[app]
/// struct MyApp {
///     priority: Select,
/// }
///
/// #[app_impl]
/// impl MyApp {
///     fn page(&self) -> Node {
///         let priorities = vec!["Low", "Medium", "High"];
///         page! {
///             column {
///                 select!(self.priority, options: priorities, placeholder: "Select priority")
///                 button(label: "Submit", on_click: Self::submit)
///             }
///         }
///     }
/// }
/// ```
#[derive(Debug)]
pub struct Select {
    /// Unique identifier for this select instance
    id: SelectId,
    /// Internal state
    inner: Arc<RwLock<SelectInner>>,
    /// Dirty flag for re-render
    dirty: Arc<AtomicBool>,
    /// Whether the dropdown is open
    is_open: Arc<AtomicBool>,
    /// Cursor position when open (for keyboard navigation)
    cursor: Arc<AtomicUsize>,
}

impl Select {
    /// Create a new select with no selection.
    pub fn new() -> Self {
        Self {
            id: SelectId::new(),
            inner: Arc::new(RwLock::new(SelectInner::default())),
            dirty: Arc::new(AtomicBool::new(false)),
            is_open: Arc::new(AtomicBool::new(false)),
            cursor: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Create a select with a placeholder.
    pub fn with_placeholder(placeholder: impl Into<String>) -> Self {
        Self {
            id: SelectId::new(),
            inner: Arc::new(RwLock::new(SelectInner {
                placeholder: placeholder.into(),
                ..Default::default()
            })),
            dirty: Arc::new(AtomicBool::new(false)),
            is_open: Arc::new(AtomicBool::new(false)),
            cursor: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Get the unique ID for this select.
    pub fn id(&self) -> SelectId {
        self.id
    }

    /// Get the ID as a string (for node binding).
    pub fn id_string(&self) -> String {
        self.id.to_string()
    }

    // -------------------------------------------------------------------------
    // Selection state
    // -------------------------------------------------------------------------

    /// Get the currently selected index.
    pub fn selected_index(&self) -> Option<usize> {
        self.inner
            .read()
            .map(|guard| guard.selected_index)
            .unwrap_or(None)
    }

    /// Set the selected index.
    pub fn set_selected_index(&self, index: Option<usize>) {
        if let Ok(mut guard) = self.inner.write()
            && guard.selected_index != index {
                guard.selected_index = index;
                guard.error = None; // Clear error on selection change
                self.dirty.store(true, Ordering::SeqCst);
            }
    }

    /// Clear the selection.
    pub fn clear(&self) {
        self.set_selected_index(None);
    }

    /// Get the label of the currently selected option.
    pub fn selected_label(&self) -> Option<String> {
        self.inner.read().ok().and_then(|guard| {
            guard
                .selected_index
                .and_then(|idx| guard.option_labels.get(idx).cloned())
        })
    }

    /// Get the placeholder text.
    pub fn placeholder(&self) -> String {
        self.inner
            .read()
            .map(|guard| guard.placeholder.clone())
            .unwrap_or_default()
    }

    /// Set the placeholder text.
    pub fn set_placeholder(&self, placeholder: impl Into<String>) {
        if let Ok(mut guard) = self.inner.write() {
            guard.placeholder = placeholder.into();
            self.dirty.store(true, Ordering::SeqCst);
        }
    }

    // -------------------------------------------------------------------------
    // Open/close state
    // -------------------------------------------------------------------------

    /// Check if the dropdown is open.
    pub fn is_open(&self) -> bool {
        self.is_open.load(Ordering::SeqCst)
    }

    /// Open the dropdown.
    pub fn open(&self) {
        if !self.is_open.swap(true, Ordering::SeqCst) {
            // Initialize cursor to selected index or 0
            let cursor_pos = self.selected_index().unwrap_or(0);
            self.cursor.store(cursor_pos, Ordering::SeqCst);
            self.dirty.store(true, Ordering::SeqCst);
        }
    }

    /// Close the dropdown.
    pub fn close(&self) {
        if self.is_open.swap(false, Ordering::SeqCst) {
            self.dirty.store(true, Ordering::SeqCst);
        }
    }

    /// Toggle the dropdown open/closed.
    pub fn toggle(&self) {
        if self.is_open() {
            self.close();
        } else {
            self.open();
        }
    }

    // -------------------------------------------------------------------------
    // Cursor navigation (when open)
    // -------------------------------------------------------------------------

    /// Get the current cursor position.
    pub fn cursor(&self) -> usize {
        self.cursor.load(Ordering::SeqCst)
    }

    /// Set the cursor position.
    pub fn set_cursor(&self, index: usize) {
        let max = self.options_count().saturating_sub(1);
        self.cursor.store(index.min(max), Ordering::SeqCst);
        self.dirty.store(true, Ordering::SeqCst);
    }

    /// Move cursor up.
    pub fn cursor_up(&self) {
        let current = self.cursor();
        if current > 0 {
            self.set_cursor(current - 1);
        }
    }

    /// Move cursor down.
    pub fn cursor_down(&self) {
        let current = self.cursor();
        let max = self.options_count().saturating_sub(1);
        if current < max {
            self.set_cursor(current + 1);
        }
    }

    // -------------------------------------------------------------------------
    // Internal methods (called by render and macros)
    // -------------------------------------------------------------------------

    /// Get the number of options.
    pub fn options_count(&self) -> usize {
        self.inner
            .read()
            .map(|guard| guard.options_count)
            .unwrap_or(0)
    }

    /// Set the number of options (called during render or by macro).
    pub fn set_options_count(&self, count: usize) {
        if let Ok(mut guard) = self.inner.write() {
            guard.options_count = count;
        }
    }

    /// Set the option labels (called during render or by macro).
    pub fn set_option_labels(&self, labels: Vec<String>) {
        if let Ok(mut guard) = self.inner.write() {
            guard.option_labels = labels;
        }
    }

    /// Get the anchor rect for overlay positioning.
    pub fn anchor_rect(&self) -> Option<Rect> {
        self.inner
            .read()
            .map(|guard| guard.anchor_rect)
            .unwrap_or(None)
    }

    /// Set the anchor rect (called during render).
    pub(crate) fn set_anchor_rect(&self, rect: Rect) {
        if let Ok(mut guard) = self.inner.write() {
            guard.anchor_rect = Some(rect);
        }
    }

    // -------------------------------------------------------------------------
    // Dirty tracking
    // -------------------------------------------------------------------------

    /// Check if the select state has changed.
    pub fn is_dirty(&self) -> bool {
        self.dirty.load(Ordering::SeqCst)
    }

    /// Clear the dirty flag.
    pub fn clear_dirty(&self) {
        self.dirty.store(false, Ordering::SeqCst);
    }

    // -------------------------------------------------------------------------
    // Validation
    // -------------------------------------------------------------------------

    /// Set a validation error message.
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

    /// Check if this select has a validation error.
    pub fn has_error(&self) -> bool {
        self.inner
            .read()
            .map(|guard| guard.error.is_some())
            .unwrap_or(false)
    }

    /// Get the current validation error message.
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

impl Clone for Select {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            inner: Arc::clone(&self.inner),
            dirty: Arc::clone(&self.dirty),
            is_open: Arc::clone(&self.is_open),
            cursor: Arc::clone(&self.cursor),
        }
    }
}

impl Default for Select {
    fn default() -> Self {
        Self::new()
    }
}

// -----------------------------------------------------------------------------
// Validatable implementation
// -----------------------------------------------------------------------------

use crate::validation::Validatable;

impl Validatable for Select {
    type Value = Option<usize>;

    fn validation_value(&self) -> Self::Value {
        self.selected_index()
    }

    fn set_error(&self, msg: impl Into<String>) {
        Select::set_error(self, msg)
    }

    fn clear_error(&self) {
        Select::clear_error(self)
    }

    fn has_error(&self) -> bool {
        Select::has_error(self)
    }

    fn error(&self) -> Option<String> {
        Select::error(self)
    }

    fn widget_id(&self) -> String {
        self.id_string()
    }

    fn error_display(&self) -> ErrorDisplay {
        Select::error_display(self)
    }

    fn set_error_display(&self, display: ErrorDisplay) {
        Select::set_error_display(self, display)
    }
}
