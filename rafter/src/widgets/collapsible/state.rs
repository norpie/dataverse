//! Collapsible widget state.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};

/// Unique identifier for a Collapsible widget instance
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CollapsibleId(usize);

impl CollapsibleId {
    fn new() -> Self {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        Self(COUNTER.fetch_add(1, Ordering::SeqCst))
    }
}

impl std::fmt::Display for CollapsibleId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "__collapsible_{}", self.0)
    }
}

/// Internal state for a Collapsible widget
#[derive(Debug)]
struct CollapsibleInner {
    /// Whether the collapsible is expanded
    expanded: bool,
    /// Header title text
    title: String,
    /// Character to display when expanded
    expanded_char: char,
    /// Character to display when collapsed
    collapsed_char: char,
}

impl Default for CollapsibleInner {
    fn default() -> Self {
        Self {
            expanded: false,
            title: String::new(),
            expanded_char: '▼',
            collapsed_char: '▶',
        }
    }
}

/// A collapsible container widget with reactive state.
///
/// `Collapsible` is a container that can expand/collapse to show/hide its content.
/// It renders a header row with an indicator (▶/▼) and conditionally displays
/// children based on its expanded state.
///
/// # Example
///
/// ```ignore
/// #[app]
/// struct MyApp {
///     details: Collapsible,
/// }
///
/// #[app_impl]
/// impl MyApp {
///     fn page(&self) -> Node {
///         page! {
///             column {
///                 collapsible(bind: self.details, title: "Details", on_expand: Self::load_details) {
///                     text { "Detail content here..." }
///                 }
///             }
///         }
///     }
///
///     #[handler]
///     async fn load_details(&self, cx: &AppContext) {
///         // Load data when expanded
///     }
/// }
/// ```
#[derive(Debug)]
pub struct Collapsible {
    /// Unique identifier for this collapsible instance
    id: CollapsibleId,
    /// Internal state
    inner: Arc<RwLock<CollapsibleInner>>,
    /// Dirty flag for re-render
    dirty: Arc<AtomicBool>,
    /// Focus request flag (checked by runtime)
    focus_requested: Arc<AtomicBool>,
}

impl Collapsible {
    /// Create a new collapsed collapsible without a title
    pub fn new() -> Self {
        Self {
            id: CollapsibleId::new(),
            inner: Arc::new(RwLock::new(CollapsibleInner::default())),
            dirty: Arc::new(AtomicBool::new(false)),
            focus_requested: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Create a collapsible with a title
    pub fn with_title(title: impl Into<String>) -> Self {
        Self {
            id: CollapsibleId::new(),
            inner: Arc::new(RwLock::new(CollapsibleInner {
                title: title.into(),
                ..Default::default()
            })),
            dirty: Arc::new(AtomicBool::new(false)),
            focus_requested: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Create an expanded collapsible
    pub fn expanded() -> Self {
        Self {
            id: CollapsibleId::new(),
            inner: Arc::new(RwLock::new(CollapsibleInner {
                expanded: true,
                ..Default::default()
            })),
            dirty: Arc::new(AtomicBool::new(false)),
            focus_requested: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Set custom indicator characters
    pub fn with_indicators(self, expanded: char, collapsed: char) -> Self {
        if let Ok(mut guard) = self.inner.write() {
            guard.expanded_char = expanded;
            guard.collapsed_char = collapsed;
        }
        self
    }

    /// Get the unique ID for this collapsible
    pub fn id(&self) -> CollapsibleId {
        self.id
    }

    /// Get the ID as a string (for node binding)
    pub fn id_string(&self) -> String {
        self.id.to_string()
    }

    // -------------------------------------------------------------------------
    // Read methods
    // -------------------------------------------------------------------------

    /// Check if the collapsible is expanded
    pub fn is_expanded(&self) -> bool {
        self.inner
            .read()
            .map(|guard| guard.expanded)
            .unwrap_or(false)
    }

    /// Get the title text
    pub fn title(&self) -> String {
        self.inner
            .read()
            .map(|guard| guard.title.clone())
            .unwrap_or_default()
    }

    /// Get the expanded indicator character
    pub fn expanded_char(&self) -> char {
        self.inner
            .read()
            .map(|guard| guard.expanded_char)
            .unwrap_or('▼')
    }

    /// Get the collapsed indicator character
    pub fn collapsed_char(&self) -> char {
        self.inner
            .read()
            .map(|guard| guard.collapsed_char)
            .unwrap_or('▶')
    }

    // -------------------------------------------------------------------------
    // Write methods
    // -------------------------------------------------------------------------

    /// Set the expanded state
    pub fn set_expanded(&self, expanded: bool) {
        if let Ok(mut guard) = self.inner.write()
            && guard.expanded != expanded
        {
            guard.expanded = expanded;
            self.dirty.store(true, Ordering::SeqCst);
        }
    }

    /// Expand the collapsible
    pub fn expand(&self) {
        self.set_expanded(true);
    }

    /// Collapse the collapsible
    pub fn collapse(&self) {
        self.set_expanded(false);
    }

    /// Toggle the expanded state
    pub fn toggle(&self) {
        if let Ok(mut guard) = self.inner.write() {
            guard.expanded = !guard.expanded;
            self.dirty.store(true, Ordering::SeqCst);
        }
    }

    /// Set the title text
    pub fn set_title(&self, title: impl Into<String>) {
        if let Ok(mut guard) = self.inner.write() {
            guard.title = title.into();
            self.dirty.store(true, Ordering::SeqCst);
        }
    }

    /// Set the indicator characters
    pub fn set_indicators(&self, expanded: char, collapsed: char) {
        if let Ok(mut guard) = self.inner.write() {
            guard.expanded_char = expanded;
            guard.collapsed_char = collapsed;
            self.dirty.store(true, Ordering::SeqCst);
        }
    }

    // -------------------------------------------------------------------------
    // Focus control
    // -------------------------------------------------------------------------

    /// Request focus for this collapsible
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

    /// Check if the collapsible state has changed
    pub fn is_dirty(&self) -> bool {
        self.dirty.load(Ordering::SeqCst)
    }

    /// Clear the dirty flag
    pub fn clear_dirty(&self) {
        self.dirty.store(false, Ordering::SeqCst);
    }
}

impl Clone for Collapsible {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            inner: Arc::clone(&self.inner),
            dirty: Arc::clone(&self.dirty),
            focus_requested: Arc::clone(&self.focus_requested),
        }
    }
}

impl Default for Collapsible {
    fn default() -> Self {
        Self::new()
    }
}
