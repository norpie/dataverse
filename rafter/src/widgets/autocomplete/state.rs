//! Autocomplete widget state.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};

use ratatui::layout::Rect;

use crate::validation::ErrorDisplay;

use super::AutocompleteItem;
use super::filter::{FilterMatch, fuzzy_filter};

/// Unique identifier for an Autocomplete widget instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AutocompleteId(usize);

impl AutocompleteId {
    fn new() -> Self {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        Self(COUNTER.fetch_add(1, Ordering::SeqCst))
    }
}

impl std::fmt::Display for AutocompleteId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "__autocomplete_{}", self.0)
    }
}

/// Internal state for an Autocomplete widget.
#[derive(Debug, Default)]
struct AutocompleteInner {
    // Input state
    /// Current text value
    value: String,
    /// Cursor position in text (byte offset)
    text_cursor: usize,
    /// Placeholder text
    placeholder: String,

    // Dropdown state
    /// Filtered item indices with scores (indices into option_labels)
    filtered: Vec<FilterMatch>,
    /// Cached anchor rect for overlay positioning
    anchor_rect: Option<Rect>,

    // Options
    /// All available option labels
    option_labels: Vec<String>,

    // Validation
    /// Validation error message (if any)
    error: Option<String>,
    /// How to display validation errors
    error_display: ErrorDisplay,
}

/// A text input with fuzzy-filtered dropdown suggestions.
///
/// `Autocomplete` combines text input functionality with a dropdown overlay
/// showing filtered suggestions based on the current input value.
///
/// # Example
///
/// ```ignore
/// #[app]
/// struct MyApp {
///     search: Autocomplete,
/// }
///
/// #[app_impl]
/// impl MyApp {
///     fn page(&self) -> Node {
///         page! {
///             column {
///                 autocomplete(
///                     bind: self.search,
///                     placeholder: "Search countries...",
///                     on_select: Self::on_country_selected,
///                 ) {
///                     text("United States")
///                     text("United Kingdom")
///                     text("Canada")
///                 }
///             }
///         }
///     }
/// }
/// ```
#[derive(Debug)]
pub struct Autocomplete {
    /// Unique identifier for this autocomplete instance
    id: AutocompleteId,
    /// Internal state
    inner: Arc<RwLock<AutocompleteInner>>,
    /// Dirty flag for re-render
    dirty: Arc<AtomicBool>,
    /// Whether the dropdown is open
    is_open: Arc<AtomicBool>,
    /// Cursor position in dropdown (for keyboard navigation)
    cursor: Arc<AtomicUsize>,
    /// Focus request flag (checked by runtime)
    focus_requested: Arc<AtomicBool>,
}

impl Autocomplete {
    /// Create a new empty autocomplete.
    pub fn new() -> Self {
        Self {
            id: AutocompleteId::new(),
            inner: Arc::new(RwLock::new(AutocompleteInner::default())),
            dirty: Arc::new(AtomicBool::new(false)),
            is_open: Arc::new(AtomicBool::new(false)),
            cursor: Arc::new(AtomicUsize::new(0)),
            focus_requested: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Create an autocomplete with a placeholder.
    pub fn with_placeholder(placeholder: impl Into<String>) -> Self {
        Self {
            id: AutocompleteId::new(),
            inner: Arc::new(RwLock::new(AutocompleteInner {
                placeholder: placeholder.into(),
                ..Default::default()
            })),
            dirty: Arc::new(AtomicBool::new(false)),
            is_open: Arc::new(AtomicBool::new(false)),
            cursor: Arc::new(AtomicUsize::new(0)),
            focus_requested: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Create an autocomplete with an initial value.
    pub fn with_value(value: impl Into<String>) -> Self {
        let value = value.into();
        let text_cursor = value.len();
        Self {
            id: AutocompleteId::new(),
            inner: Arc::new(RwLock::new(AutocompleteInner {
                value,
                text_cursor,
                ..Default::default()
            })),
            dirty: Arc::new(AtomicBool::new(false)),
            is_open: Arc::new(AtomicBool::new(false)),
            cursor: Arc::new(AtomicUsize::new(0)),
            focus_requested: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Get the unique ID for this autocomplete.
    pub fn id(&self) -> AutocompleteId {
        self.id
    }

    /// Get the ID as a string (for node binding).
    pub fn id_string(&self) -> String {
        self.id.to_string()
    }

    // -------------------------------------------------------------------------
    // Text value (Input-like)
    // -------------------------------------------------------------------------

    /// Get the current text value.
    pub fn value(&self) -> String {
        self.inner
            .read()
            .map(|guard| guard.value.clone())
            .unwrap_or_default()
    }

    /// Set the text value.
    pub fn set_value(&self, value: impl Into<String>) {
        if let Ok(mut guard) = self.inner.write() {
            guard.value = value.into();
            guard.text_cursor = guard.value.len();
            guard.error = None;
            self.refilter_locked(&mut guard);
            self.dirty.store(true, Ordering::SeqCst);
        }
    }

    /// Clear the text value.
    pub fn clear(&self) {
        if let Ok(mut guard) = self.inner.write() {
            guard.value.clear();
            guard.text_cursor = 0;
            guard.error = None;
            self.refilter_locked(&mut guard);
            self.dirty.store(true, Ordering::SeqCst);
        }
    }

    /// Check if the input is empty.
    pub fn is_empty(&self) -> bool {
        self.inner
            .read()
            .map(|guard| guard.value.is_empty())
            .unwrap_or(true)
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

    /// Get the text cursor position (byte offset).
    pub fn text_cursor(&self) -> usize {
        self.inner
            .read()
            .map(|guard| guard.text_cursor)
            .unwrap_or(0)
    }

    // -------------------------------------------------------------------------
    // Text manipulation (called by runtime on key events)
    // -------------------------------------------------------------------------

    /// Insert a character at the cursor position.
    pub fn insert_char(&self, c: char) {
        if let Ok(mut guard) = self.inner.write() {
            let cursor = guard.text_cursor;
            guard.value.insert(cursor, c);
            guard.text_cursor += c.len_utf8();
            guard.error = None;
            self.refilter_locked(&mut guard);
            self.dirty.store(true, Ordering::SeqCst);
        }
    }

    /// Delete the character before the cursor (backspace).
    pub fn delete_char_before(&self) {
        if let Ok(mut guard) = self.inner.write()
            && guard.text_cursor > 0
        {
            let prev_cursor = guard.value[..guard.text_cursor]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
            guard.value.remove(prev_cursor);
            guard.text_cursor = prev_cursor;
            guard.error = None;
            self.refilter_locked(&mut guard);
            self.dirty.store(true, Ordering::SeqCst);
        }
    }

    /// Delete the character at the cursor (delete key).
    pub fn delete_char_at(&self) {
        if let Ok(mut guard) = self.inner.write() {
            let cursor = guard.text_cursor;
            if cursor < guard.value.len() {
                guard.value.remove(cursor);
                guard.error = None;
                self.refilter_locked(&mut guard);
                self.dirty.store(true, Ordering::SeqCst);
            }
        }
    }

    /// Move text cursor left.
    pub fn text_cursor_left(&self) {
        if let Ok(mut guard) = self.inner.write()
            && guard.text_cursor > 0
        {
            guard.text_cursor = guard.value[..guard.text_cursor]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.dirty.store(true, Ordering::SeqCst);
        }
    }

    /// Move text cursor right.
    pub fn text_cursor_right(&self) {
        if let Ok(mut guard) = self.inner.write()
            && guard.text_cursor < guard.value.len()
        {
            guard.text_cursor = guard.value[guard.text_cursor..]
                .char_indices()
                .nth(1)
                .map(|(i, _)| guard.text_cursor + i)
                .unwrap_or(guard.value.len());
            self.dirty.store(true, Ordering::SeqCst);
        }
    }

    /// Move text cursor to start.
    pub fn text_cursor_home(&self) {
        if let Ok(mut guard) = self.inner.write()
            && guard.text_cursor != 0
        {
            guard.text_cursor = 0;
            self.dirty.store(true, Ordering::SeqCst);
        }
    }

    /// Move text cursor to end.
    pub fn text_cursor_end(&self) {
        if let Ok(mut guard) = self.inner.write() {
            let end = guard.value.len();
            if guard.text_cursor != end {
                guard.text_cursor = end;
                self.dirty.store(true, Ordering::SeqCst);
            }
        }
    }

    // -------------------------------------------------------------------------
    // Dropdown open/close state
    // -------------------------------------------------------------------------

    /// Check if the dropdown is open.
    pub fn is_open(&self) -> bool {
        self.is_open.load(Ordering::SeqCst)
    }

    /// Open the dropdown.
    pub fn open(&self) {
        if !self.is_open.swap(true, Ordering::SeqCst) {
            // Reset cursor to top
            self.cursor.store(0, Ordering::SeqCst);
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
    // Dropdown cursor navigation
    // -------------------------------------------------------------------------

    /// Get the current dropdown cursor position.
    pub fn cursor(&self) -> usize {
        self.cursor.load(Ordering::SeqCst)
    }

    /// Set the dropdown cursor position.
    pub fn set_cursor(&self, index: usize) {
        let max = self.filtered_count().saturating_sub(1);
        self.cursor.store(index.min(max), Ordering::SeqCst);
        self.dirty.store(true, Ordering::SeqCst);
    }

    /// Move dropdown cursor up.
    pub fn cursor_up(&self) {
        let current = self.cursor();
        if current > 0 {
            self.set_cursor(current - 1);
        }
    }

    /// Move dropdown cursor down.
    pub fn cursor_down(&self) {
        let current = self.cursor();
        let max = self.filtered_count().saturating_sub(1);
        if current < max {
            self.set_cursor(current + 1);
        }
    }

    /// Select the item at the current cursor position.
    /// Sets the input value to the selected item's label.
    pub fn select_at_cursor(&self) {
        let cursor = self.cursor();
        if let Ok(guard) = self.inner.read()
            && let Some(filter_match) = guard.filtered.get(cursor)
                && let Some(label) = guard.option_labels.get(filter_match.index) {
                    let label = label.clone();
                    drop(guard);
                    self.set_value(label);
                    self.close();
                }
    }

    // -------------------------------------------------------------------------
    // Items management
    // -------------------------------------------------------------------------

    /// Set the available items dynamically (for async loading).
    pub fn set_items<I: AutocompleteItem>(&self, items: &[I]) {
        if let Ok(mut guard) = self.inner.write() {
            guard.option_labels = items.iter().map(|i| i.autocomplete_label()).collect();
            self.refilter_locked(&mut guard);
            self.dirty.store(true, Ordering::SeqCst);
        }
    }

    /// Set option labels directly (called during render).
    pub fn set_option_labels(&self, labels: Vec<String>) {
        if let Ok(mut guard) = self.inner.write() {
            guard.option_labels = labels;
            self.refilter_locked(&mut guard);
        }
    }

    /// Get the number of filtered items.
    pub fn filtered_count(&self) -> usize {
        self.inner
            .read()
            .map(|guard| guard.filtered.len())
            .unwrap_or(0)
    }

    /// Get the filtered items (indices and scores).
    pub fn filtered(&self) -> Vec<FilterMatch> {
        self.inner
            .read()
            .map(|guard| guard.filtered.clone())
            .unwrap_or_default()
    }

    /// Get the label at a filtered index.
    pub fn filtered_label(&self, filtered_index: usize) -> Option<String> {
        self.inner.read().ok().and_then(|guard| {
            guard
                .filtered
                .get(filtered_index)
                .and_then(|m| guard.option_labels.get(m.index).cloned())
        })
    }

    /// Re-run the fuzzy filter with current value.
    fn refilter_locked(&self, guard: &mut AutocompleteInner) {
        guard.filtered = fuzzy_filter(&guard.value, &guard.option_labels);
        // Reset cursor if out of bounds
        let max = guard.filtered.len().saturating_sub(1);
        let current = self.cursor.load(Ordering::SeqCst);
        if current > max {
            self.cursor.store(0, Ordering::SeqCst);
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
    // Focus control
    // -------------------------------------------------------------------------

    /// Request focus for this autocomplete.
    pub fn focus(&self) {
        self.focus_requested.store(true, Ordering::SeqCst);
    }

    /// Check and clear the focus request (called by runtime).
    pub fn take_focus_request(&self) -> bool {
        self.focus_requested.swap(false, Ordering::SeqCst)
    }

    // -------------------------------------------------------------------------
    // Dirty tracking
    // -------------------------------------------------------------------------

    /// Check if the autocomplete state has changed.
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
            && guard.error.is_some()
        {
            guard.error = None;
            self.dirty.store(true, Ordering::SeqCst);
        }
    }

    /// Check if this autocomplete has a validation error.
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

impl Clone for Autocomplete {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            inner: Arc::clone(&self.inner),
            dirty: Arc::clone(&self.dirty),
            is_open: Arc::clone(&self.is_open),
            cursor: Arc::clone(&self.cursor),
            focus_requested: Arc::clone(&self.focus_requested),
        }
    }
}

impl Default for Autocomplete {
    fn default() -> Self {
        Self::new()
    }
}

// -----------------------------------------------------------------------------
// Validatable implementation
// -----------------------------------------------------------------------------

use crate::validation::Validatable;

impl Validatable for Autocomplete {
    type Value = String;

    fn validation_value(&self) -> Self::Value {
        self.value()
    }

    fn set_error(&self, msg: impl Into<String>) {
        Autocomplete::set_error(self, msg)
    }

    fn clear_error(&self) {
        Autocomplete::clear_error(self)
    }

    fn has_error(&self) -> bool {
        Autocomplete::has_error(self)
    }

    fn error(&self) -> Option<String> {
        Autocomplete::error(self)
    }

    fn widget_id(&self) -> String {
        self.id_string()
    }

    fn error_display(&self) -> ErrorDisplay {
        Autocomplete::error_display(self)
    }

    fn set_error_display(&self, display: ErrorDisplay) {
        Autocomplete::set_error_display(self, display)
    }
}
