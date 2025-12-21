//! Checkbox widget state.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};

/// Unique identifier for a Checkbox widget instance
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CheckboxId(usize);

impl CheckboxId {
    fn new() -> Self {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        Self(COUNTER.fetch_add(1, Ordering::SeqCst))
    }
}

impl std::fmt::Display for CheckboxId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "__checkbox_{}", self.0)
    }
}

/// Internal state for a Checkbox widget
#[derive(Debug)]
struct CheckboxInner {
    /// Whether the checkbox is checked
    checked: bool,
    /// Label text
    label: String,
    /// Character to display when checked
    checked_char: char,
    /// Character to display when unchecked
    unchecked_char: char,
}

impl Default for CheckboxInner {
    fn default() -> Self {
        Self {
            checked: false,
            label: String::new(),
            checked_char: '■',
            unchecked_char: '□',
        }
    }
}

/// A checkbox widget with reactive state.
///
/// `Checkbox` is a self-contained widget that manages its own checked state.
/// It provides imperative methods for reading and modifying the state programmatically.
///
/// # Example
///
/// ```ignore
/// #[app]
/// struct MyApp {
///     accept_terms: Checkbox,
/// }
///
/// #[app_impl]
/// impl MyApp {
///     fn page(&self) -> Node {
///         page! {
///             column {
///                 checkbox(bind: self.accept_terms, label: "I accept the terms")
///                 button(label: "Submit", on_click: Self::submit)
///             }
///         }
///     }
///
///     #[handler]
///     async fn submit(&self, cx: &AppContext) {
///         if self.accept_terms.is_checked() {
///             // ... proceed
///         }
///     }
/// }
/// ```
#[derive(Debug)]
pub struct Checkbox {
    /// Unique identifier for this checkbox instance
    id: CheckboxId,
    /// Internal state
    inner: Arc<RwLock<CheckboxInner>>,
    /// Dirty flag for re-render
    dirty: Arc<AtomicBool>,
    /// Focus request flag (checked by runtime)
    focus_requested: Arc<AtomicBool>,
}

impl Checkbox {
    /// Create a new unchecked checkbox without a label
    pub fn new() -> Self {
        Self {
            id: CheckboxId::new(),
            inner: Arc::new(RwLock::new(CheckboxInner::default())),
            dirty: Arc::new(AtomicBool::new(false)),
            focus_requested: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Create a checkbox with a label
    pub fn with_label(label: impl Into<String>) -> Self {
        Self {
            id: CheckboxId::new(),
            inner: Arc::new(RwLock::new(CheckboxInner {
                label: label.into(),
                ..Default::default()
            })),
            dirty: Arc::new(AtomicBool::new(false)),
            focus_requested: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Create a checked checkbox
    pub fn checked() -> Self {
        Self {
            id: CheckboxId::new(),
            inner: Arc::new(RwLock::new(CheckboxInner {
                checked: true,
                ..Default::default()
            })),
            dirty: Arc::new(AtomicBool::new(false)),
            focus_requested: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Set custom indicator characters
    pub fn with_indicators(self, checked: char, unchecked: char) -> Self {
        if let Ok(mut guard) = self.inner.write() {
            guard.checked_char = checked;
            guard.unchecked_char = unchecked;
        }
        self
    }

    /// Get the unique ID for this checkbox
    pub fn id(&self) -> CheckboxId {
        self.id
    }

    /// Get the ID as a string (for node binding)
    pub fn id_string(&self) -> String {
        self.id.to_string()
    }

    // -------------------------------------------------------------------------
    // Read methods
    // -------------------------------------------------------------------------

    /// Check if the checkbox is checked
    pub fn is_checked(&self) -> bool {
        self.inner
            .read()
            .map(|guard| guard.checked)
            .unwrap_or(false)
    }

    /// Get the label text
    pub fn label(&self) -> String {
        self.inner
            .read()
            .map(|guard| guard.label.clone())
            .unwrap_or_default()
    }

    /// Get the checked indicator character
    pub fn checked_char(&self) -> char {
        self.inner
            .read()
            .map(|guard| guard.checked_char)
            .unwrap_or('■')
    }

    /// Get the unchecked indicator character
    pub fn unchecked_char(&self) -> char {
        self.inner
            .read()
            .map(|guard| guard.unchecked_char)
            .unwrap_or('□')
    }

    // -------------------------------------------------------------------------
    // Write methods
    // -------------------------------------------------------------------------

    /// Set the checked state
    pub fn set_checked(&self, checked: bool) {
        if let Ok(mut guard) = self.inner.write()
            && guard.checked != checked
        {
            guard.checked = checked;
            self.dirty.store(true, Ordering::SeqCst);
        }
    }

    /// Toggle the checked state
    pub fn toggle(&self) {
        if let Ok(mut guard) = self.inner.write() {
            guard.checked = !guard.checked;
            self.dirty.store(true, Ordering::SeqCst);
        }
    }

    /// Set the label text
    pub fn set_label(&self, label: impl Into<String>) {
        if let Ok(mut guard) = self.inner.write() {
            guard.label = label.into();
            self.dirty.store(true, Ordering::SeqCst);
        }
    }

    /// Set the indicator characters
    pub fn set_indicators(&self, checked: char, unchecked: char) {
        if let Ok(mut guard) = self.inner.write() {
            guard.checked_char = checked;
            guard.unchecked_char = unchecked;
            self.dirty.store(true, Ordering::SeqCst);
        }
    }

    // -------------------------------------------------------------------------
    // Focus control
    // -------------------------------------------------------------------------

    /// Request focus for this checkbox
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

    /// Check if the checkbox state has changed
    pub fn is_dirty(&self) -> bool {
        self.dirty.load(Ordering::SeqCst)
    }

    /// Clear the dirty flag
    pub fn clear_dirty(&self) {
        self.dirty.store(false, Ordering::SeqCst);
    }
}

impl Clone for Checkbox {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            inner: Arc::clone(&self.inner),
            dirty: Arc::clone(&self.dirty),
            focus_requested: Arc::clone(&self.focus_requested),
        }
    }
}

impl Default for Checkbox {
    fn default() -> Self {
        Self::new()
    }
}
