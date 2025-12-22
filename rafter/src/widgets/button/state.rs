//! Button widget state.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};

/// Generate a unique auto-incremented button ID
fn generate_auto_id() -> String {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("__button_{}", id)
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
    id: String,
    /// Internal state
    inner: Arc<RwLock<ButtonInner>>,
    /// Dirty flag for re-render
    dirty: Arc<AtomicBool>,
}

impl Button {
    /// Create a new button with the given label (auto-generated ID)
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            id: generate_auto_id(),
            inner: Arc::new(RwLock::new(ButtonInner {
                label: label.into(),
            })),
            dirty: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Create a new button with a custom ID and label
    pub fn with_id(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            inner: Arc::new(RwLock::new(ButtonInner {
                label: label.into(),
            })),
            dirty: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Get the ID as a string (for node binding)
    pub fn id_string(&self) -> String {
        self.id.clone()
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
            id: self.id.clone(),
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
