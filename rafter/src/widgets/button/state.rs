//! Button widget state.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};

/// Unique identifier for a Button widget instance
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ButtonId(usize);

impl ButtonId {
    fn new() -> Self {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        Self(COUNTER.fetch_add(1, Ordering::SeqCst))
    }
}

impl std::fmt::Display for ButtonId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "__button_{}", self.0)
    }
}

/// Internal state for a Button widget
#[derive(Debug)]
struct ButtonInner {
    /// Button label text
    label: String,
}

/// A button widget.
///
/// `Button` is a simple clickable widget that displays a label.
/// It triggers an `on_click` or `on_activate` handler when clicked or
/// when Enter is pressed while focused.
///
/// Unlike Input or Checkbox, Button has no mutable internal state beyond
/// its label. The click handler is stored in `WidgetHandlers` on the Node.
///
/// # Example
///
/// ```ignore
/// #[app]
/// struct MyApp {
///     submit_button: Button,
/// }
///
/// #[app_impl]
/// impl MyApp {
///     fn page(&self) -> Node {
///         page! {
///             column {
///                 button(bind: self.submit_button, on_click: Self::submit)
///             }
///         }
///     }
///
///     #[handler]
///     async fn submit(&self, cx: &AppContext) {
///         // Handle button click
///     }
/// }
/// ```
#[derive(Debug)]
pub struct Button {
    /// Unique identifier for this button instance
    id: ButtonId,
    /// Internal state
    inner: Arc<RwLock<ButtonInner>>,
    /// Dirty flag for re-render
    dirty: Arc<AtomicBool>,
}

impl Button {
    /// Create a new button with the given label
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            id: ButtonId::new(),
            inner: Arc::new(RwLock::new(ButtonInner {
                label: label.into(),
            })),
            dirty: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Get the unique ID for this button
    pub fn id(&self) -> ButtonId {
        self.id
    }

    /// Get the ID as a string (for node binding)
    pub fn id_string(&self) -> String {
        self.id.to_string()
    }

    // -------------------------------------------------------------------------
    // Read methods
    // -------------------------------------------------------------------------

    /// Get the button label
    pub fn label(&self) -> String {
        self.inner
            .read()
            .map(|guard| guard.label.clone())
            .unwrap_or_default()
    }

    // -------------------------------------------------------------------------
    // Write methods
    // -------------------------------------------------------------------------

    /// Set the button label
    pub fn set_label(&self, label: impl Into<String>) {
        if let Ok(mut guard) = self.inner.write() {
            guard.label = label.into();
            self.dirty.store(true, Ordering::SeqCst);
        }
    }

    // -------------------------------------------------------------------------
    // Dirty tracking
    // -------------------------------------------------------------------------

    /// Check if the button state has changed
    pub fn is_dirty(&self) -> bool {
        self.dirty.load(Ordering::SeqCst)
    }

    /// Clear the dirty flag
    pub fn clear_dirty(&self) {
        self.dirty.store(false, Ordering::SeqCst);
    }
}

impl Clone for Button {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            inner: Arc::clone(&self.inner),
            dirty: Arc::clone(&self.dirty),
        }
    }
}

impl Default for Button {
    fn default() -> Self {
        Self::new("")
    }
}
