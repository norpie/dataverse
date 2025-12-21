//! Radio group component state.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};

/// Unique identifier for a RadioGroup component instance
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

/// Internal state for a RadioGroup component
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
}

impl Default for RadioGroupInner {
    fn default() -> Self {
        Self {
            selected: None,
            options: Vec::new(),
            selected_char: '◉',
            unselected_char: '◯',
        }
    }
}

/// A radio group component with reactive state.
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
///     fn view(&self) -> Node {
///         view! {
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
        self.inner
            .read()
            .ok()
            .and_then(|guard| {
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
        if let Ok(mut guard) = self.inner.write() {
            if index < guard.options.len() && guard.selected != Some(index) {
                guard.selected = Some(index);
                self.dirty.store(true, Ordering::SeqCst);
            }
        }
    }

    /// Clear the selection
    pub fn clear(&self) {
        if let Ok(mut guard) = self.inner.write() {
            if guard.selected.is_some() {
                guard.selected = None;
                self.dirty.store(true, Ordering::SeqCst);
            }
        }
    }

    /// Set the available options (clears selection if now out of bounds)
    pub fn set_options(&self, options: Vec<impl Into<String>>) {
        if let Ok(mut guard) = self.inner.write() {
            guard.options = options.into_iter().map(|l| l.into()).collect();
            // Clear selection if it's no longer valid
            if let Some(idx) = guard.selected {
                if idx >= guard.options.len() {
                    guard.selected = None;
                }
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
