//! Radio group widget state.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};

use crate::validation::ErrorDisplay;

/// Unique identifier for a RadioGroup widget instance
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RadioGroupId(usize);

impl RadioGroupId {
    fn new() -> Self {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        Self(COUNTER.fetch_add(1, Ordering::SeqCst))
    }
}

impl std::fmt::Display for RadioGroupId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "__radio_group_{}", self.0)
    }
}

/// Internal state for a RadioGroup widget
#[derive(Debug)]
struct RadioGroupInner {
    /// The currently selected index (if any)
    selected: Option<usize>,
    /// Available option labels
    options: Vec<String>,
    /// Character to display when selected
    selected_char: char,
    /// Character to display when not selected
    unselected_char: char,
    /// Validation error message (if any)
    error: Option<String>,
    /// How to display validation errors
    error_display: ErrorDisplay,
}

impl Default for RadioGroupInner {
    fn default() -> Self {
        Self {
            selected: None,
            options: Vec::new(),
            selected_char: '◉',
            unselected_char: '◯',
            error: None,
            error_display: ErrorDisplay::default(),
        }
    }
}

/// A radio group widget with reactive state.
///
/// `RadioGroup` manages a group of mutually exclusive options where only one
/// can be selected at a time. Selection is index-based.
///
/// # Example
///
/// ```ignore
/// #[app]
/// struct MyApp {
///     theme: RadioGroup,
/// }
///
/// #[app_impl]
/// impl MyApp {
///     async fn on_start(&self, _cx: &AppContext) {
///         self.theme.set_options(vec!["Light", "Dark", "System"]);
///         self.theme.select(2); // Select "System"
///     }
///
///     fn page(&self) -> Node {
///         page! {
///             column {
///                 radio_group(bind: self.theme, on_change: on_theme_change)
///             }
///         }
///     }
///
///     #[handler]
///     async fn on_theme_change(&self, cx: &AppContext) {
///         match self.theme.selected() {
///             Some(0) => cx.toast("Light mode"),
///             Some(1) => cx.toast("Dark mode"),
///             Some(2) => cx.toast("System default"),
///             _ => {}
///         }
///     }
/// }
/// ```
#[derive(Debug)]
pub struct RadioGroup {
    /// Unique identifier for this radio group instance
    id: RadioGroupId,
    /// Internal state
    inner: Arc<RwLock<RadioGroupInner>>,
    /// Dirty flag for re-render
    dirty: Arc<AtomicBool>,
    /// Focus request flag (checked by runtime)
    focus_requested: Arc<AtomicBool>,
}

impl RadioGroup {
    /// Create a new empty radio group
    pub fn new() -> Self {
        Self {
            id: RadioGroupId::new(),
            inner: Arc::new(RwLock::new(RadioGroupInner::default())),
            dirty: Arc::new(AtomicBool::new(false)),
            focus_requested: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Create a radio group with options
    pub fn with_options(options: Vec<impl Into<String>>) -> Self {
        let options = options.into_iter().map(|l| l.into()).collect();
        Self {
            id: RadioGroupId::new(),
            inner: Arc::new(RwLock::new(RadioGroupInner {
                options,
                ..Default::default()
            })),
            dirty: Arc::new(AtomicBool::new(false)),
            focus_requested: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Set custom indicator characters
    pub fn with_indicators(self, selected: char, unselected: char) -> Self {
        if let Ok(mut guard) = self.inner.write() {
            guard.selected_char = selected;
            guard.unselected_char = unselected;
        }
        self
    }

    /// Get the unique ID for this radio group
    pub fn id(&self) -> RadioGroupId {
        self.id
    }

    /// Get the ID as a string (for node binding)
    pub fn id_string(&self) -> String {
        self.id.to_string()
    }

    // -------------------------------------------------------------------------
    // Read methods
    // -------------------------------------------------------------------------

    /// Get the currently selected index (if any)
    pub fn selected(&self) -> Option<usize> {
        self.inner
            .read()
            .map(|guard| guard.selected)
            .unwrap_or(None)
    }

    /// Get all option labels
    pub fn options(&self) -> Vec<String> {
        self.inner
            .read()
            .map(|guard| guard.options.clone())
            .unwrap_or_default()
    }

    /// Get the number of options
    pub fn len(&self) -> usize {
        self.inner
            .read()
            .map(|guard| guard.options.len())
            .unwrap_or(0)
    }

    /// Check if there are no options
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Check if a specific index is selected
    pub fn is_selected(&self, index: usize) -> bool {
        self.inner
            .read()
            .map(|guard| guard.selected == Some(index))
            .unwrap_or(false)
    }

    /// Get the label for an option at index
    pub fn label_at(&self, index: usize) -> Option<String> {
        self.inner
            .read()
            .ok()
            .and_then(|guard| guard.options.get(index).cloned())
    }

    /// Get the label for the currently selected option
    pub fn selected_label(&self) -> Option<String> {
        self.inner.read().ok().and_then(|guard| {
            guard
                .selected
                .and_then(|idx| guard.options.get(idx).cloned())
        })
    }

    /// Get the selected indicator character
    pub fn selected_char(&self) -> char {
        self.inner
            .read()
            .map(|guard| guard.selected_char)
            .unwrap_or('◉')
    }

    /// Get the unselected indicator character
    pub fn unselected_char(&self) -> char {
        self.inner
            .read()
            .map(|guard| guard.unselected_char)
            .unwrap_or('◯')
    }

    // -------------------------------------------------------------------------
    // Write methods
    // -------------------------------------------------------------------------

    /// Select an option by index
    pub fn select(&self, index: usize) {
        if let Ok(mut guard) = self.inner.write()
            && index < guard.options.len() && guard.selected != Some(index) {
                guard.selected = Some(index);
                guard.error = None; // Auto-clear error on value change
                self.dirty.store(true, Ordering::SeqCst);
            }
    }

    /// Clear the selection
    pub fn clear(&self) {
        if let Ok(mut guard) = self.inner.write()
            && guard.selected.is_some() {
                guard.selected = None;
                guard.error = None; // Auto-clear error on value change
                self.dirty.store(true, Ordering::SeqCst);
            }
    }

    /// Set the available options (clears selection if now out of bounds)
    pub fn set_options(&self, options: Vec<impl Into<String>>) {
        if let Ok(mut guard) = self.inner.write() {
            guard.options = options.into_iter().map(|l| l.into()).collect();
            // Clear selection if it's no longer valid
            if let Some(idx) = guard.selected
                && idx >= guard.options.len() {
                    guard.selected = None;
                }
            self.dirty.store(true, Ordering::SeqCst);
        }
    }

    /// Set the indicator characters
    pub fn set_indicators(&self, selected: char, unselected: char) {
        if let Ok(mut guard) = self.inner.write() {
            guard.selected_char = selected;
            guard.unselected_char = unselected;
            self.dirty.store(true, Ordering::SeqCst);
        }
    }

    // -------------------------------------------------------------------------
    // Focus control
    // -------------------------------------------------------------------------

    /// Request focus for this radio group
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

    /// Check if the radio group state has changed
    pub fn is_dirty(&self) -> bool {
        self.dirty.load(Ordering::SeqCst)
    }

    /// Clear the dirty flag
    pub fn clear_dirty(&self) {
        self.dirty.store(false, Ordering::SeqCst);
    }
}

impl Clone for RadioGroup {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            inner: Arc::clone(&self.inner),
            dirty: Arc::clone(&self.dirty),
            focus_requested: Arc::clone(&self.focus_requested),
        }
    }
}

impl Default for RadioGroup {
    fn default() -> Self {
        Self::new()
    }
}

impl RadioGroup {
    // -------------------------------------------------------------------------
    // Validation
    // -------------------------------------------------------------------------

    /// Set a validation error message on this radio group.
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

    /// Check if this radio group has a validation error.
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

// -----------------------------------------------------------------------------
// Validatable implementation
// -----------------------------------------------------------------------------

use crate::validation::Validatable;

impl Validatable for RadioGroup {
    /// Value type is `Option<usize>` - the selected index
    type Value = Option<usize>;

    fn validation_value(&self) -> Self::Value {
        self.selected()
    }

    fn set_error(&self, msg: impl Into<String>) {
        RadioGroup::set_error(self, msg)
    }

    fn clear_error(&self) {
        RadioGroup::clear_error(self)
    }

    fn has_error(&self) -> bool {
        RadioGroup::has_error(self)
    }

    fn error(&self) -> Option<String> {
        RadioGroup::error(self)
    }

    fn widget_id(&self) -> String {
        self.id_string()
    }

    fn error_display(&self) -> ErrorDisplay {
        RadioGroup::error_display(self)
    }

    fn set_error_display(&self, display: ErrorDisplay) {
        RadioGroup::set_error_display(self, display)
    }
}
