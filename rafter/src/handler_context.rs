//! Handler context bundle for unified context passing to handlers.
//!
//! This module provides:
//! - `HandlerContext`: bundles all available contexts for passing to handlers
//! - `Handler`: closure type for handlers
//! - `HandlerRegistry`: stores widget event handlers keyed by (element_id, event_type)
//!
//! Handlers declare what contexts they need in their signature, and the
//! `#[app_impl]` macro generates code to extract the appropriate contexts from
//! the HandlerContext bundle.

use std::any::Any;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::{AppContext, GlobalContext, ModalContext};

// =============================================================================
// Handler Type
// =============================================================================

/// A handler closure that receives a HandlerContext.
///
/// This is the unified handler type used for both keybinds and widget events.
/// The closure captures `self` and any arguments at creation time.
pub type Handler = Arc<dyn Fn(&HandlerContext) + Send + Sync>;

/// Map of handler names to handlers, used for passing callbacks to widgets.
///
/// Standard handler names:
/// - `"on_activate"` - button click, enter key, selection confirm
/// - `"on_change"` - value changed (select, checkbox, input)
/// - `"on_submit"` - form submission, enter in input
/// - `"on_focus"` - element gained focus
/// - `"on_blur"` - element lost focus
pub type WidgetHandlers = HashMap<&'static str, Handler>;

// =============================================================================
// Event Data
// =============================================================================

/// Event-specific data passed to handlers via HandlerContext.
///
/// This allows handlers to access data from the event that triggered them,
/// such as the new text value for input change events.
#[derive(Debug, Clone, Default)]
pub enum EventData {
    /// No event data (default for keybinds and events without data).
    #[default]
    None,
    /// Text input value changed.
    Change {
        /// The new text value.
        text: String,
    },
    /// Text input submitted (Enter pressed).
    Submit,
    /// Element lost focus.
    Blur {
        /// The element that received focus (if any).
        new_target: Option<String>,
    },
    /// Scroll position changed.
    Scroll {
        /// Current horizontal scroll offset.
        offset_x: u16,
        /// Current vertical scroll offset.
        offset_y: u16,
        /// Total content width.
        content_width: u16,
        /// Total content height.
        content_height: u16,
        /// Visible viewport width.
        viewport_width: u16,
        /// Visible viewport height.
        viewport_height: u16,
    },
}

impl EventData {
    /// Get the changed text from a Change event.
    pub fn text(&self) -> Option<&str> {
        match self {
            EventData::Change { text } => Some(text),
            _ => None,
        }
    }

    /// Get the new focus target from a Blur event.
    pub fn blur_target(&self) -> Option<&str> {
        match self {
            EventData::Blur { new_target } => new_target.as_deref(),
            _ => None,
        }
    }

    /// Get scroll offset (x, y) from a Scroll event.
    pub fn scroll_offset(&self) -> Option<(u16, u16)> {
        match self {
            EventData::Scroll { offset_x, offset_y, .. } => Some((*offset_x, *offset_y)),
            _ => None,
        }
    }

    /// Get vertical scroll progress (0.0 = top, 1.0 = bottom).
    pub fn scroll_progress_y(&self) -> Option<f32> {
        match self {
            EventData::Scroll { offset_y, content_height, viewport_height, .. } => {
                let max_scroll = content_height.saturating_sub(*viewport_height);
                if max_scroll == 0 {
                    Some(0.0)
                } else {
                    Some(*offset_y as f32 / max_scroll as f32)
                }
            }
            _ => None,
        }
    }

    /// Check if scroll position is near the bottom.
    ///
    /// Returns true if within `threshold` pixels of the bottom, or if not a Scroll event.
    pub fn is_near_bottom(&self, threshold: u16) -> bool {
        match self {
            EventData::Scroll { offset_y, content_height, viewport_height, .. } => {
                let max_scroll = content_height.saturating_sub(*viewport_height);
                *offset_y + threshold >= max_scroll
            }
            _ => false,
        }
    }

    /// Check if scroll position is near the top.
    ///
    /// Returns true if within `threshold` pixels of the top, or if not a Scroll event.
    pub fn is_near_top(&self, threshold: u16) -> bool {
        match self {
            EventData::Scroll { offset_y, .. } => *offset_y <= threshold,
            _ => false,
        }
    }
}

// =============================================================================
// HandlerRegistry
// =============================================================================

/// Registry for widget event handlers.
///
/// Maps (element_id, event_type) to handler closures. Cleared at the start
/// of each `element()` call to ensure handlers from previous renders don't persist.
#[derive(Default, Clone)]
pub struct HandlerRegistry {
    handlers: Arc<RwLock<HashMap<(String, String), Handler>>>,
}

impl HandlerRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a handler for an element event.
    ///
    /// # Arguments
    /// - `element_id`: The element's unique ID (from Element.id)
    /// - `event`: The event type (e.g., "on_click", "on_change")
    /// - `handler`: The handler closure
    pub fn register(&self, element_id: &str, event: &str, handler: Handler) {
        if let Ok(mut handlers) = self.handlers.write() {
            handlers.insert((element_id.to_string(), event.to_string()), handler);
        }
    }

    /// Get a handler for an element event.
    pub fn get(&self, element_id: &str, event: &str) -> Option<Handler> {
        self.handlers
            .read()
            .ok()?
            .get(&(element_id.to_string(), event.to_string()))
            .cloned()
    }

    /// Clear all handlers.
    ///
    /// Called at the start of `element()` to remove handlers from previous renders.
    pub fn clear(&self) {
        if let Ok(mut handlers) = self.handlers.write() {
            handlers.clear();
        }
    }

    /// Check if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.handlers
            .read()
            .map(|h| h.is_empty())
            .unwrap_or(true)
    }

    /// Get the number of registered handlers.
    pub fn len(&self) -> usize {
        self.handlers.read().map(|h| h.len()).unwrap_or(0)
    }
}

impl std::fmt::Debug for HandlerRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let count = self.len();
        f.debug_struct("HandlerRegistry")
            .field("handler_count", &count)
            .finish()
    }
}

/// Context bundle passed to all widget handlers (inline and keybind).
///
/// This struct provides unified access to all available contexts. The
/// `#[app_impl]` macro generates closures that accept `&HandlerContext`
/// and extract the specific contexts each handler needs.
///
/// # For Apps
///
/// App handlers have access to `cx()` and `gx()`. Attempting to use `mx()`
/// will panic (but compile-time checks in `#[app_impl]` prevent this).
///
/// # For Modals
///
/// Modal handlers have access to `cx()`, `gx()`, and `mx()`.
///
/// # For Systems
///
/// System handlers only have access to `gx()`. Attempting to use `cx()`
/// will panic (but compile-time checks in `#[system_impl]` prevent this).
pub struct HandlerContext<'a> {
    /// App context (None for systems)
    cx: Option<&'a AppContext>,
    gx: &'a GlobalContext,
    /// Type-erased modal context (None for apps/systems)
    modal_context: Option<&'a (dyn Any + Send + Sync)>,
    /// Event-specific data (for widget event handlers)
    event_data: EventData,
}

impl<'a> HandlerContext<'a> {
    /// Create a HandlerContext for app handlers (no modal context).
    pub fn for_app(cx: &'a AppContext, gx: &'a GlobalContext) -> Self {
        Self {
            cx: Some(cx),
            gx,
            modal_context: None,
            event_data: EventData::None,
        }
    }

    /// Create a HandlerContext for app handlers with event data.
    pub fn for_app_with_event(
        cx: &'a AppContext,
        gx: &'a GlobalContext,
        event_data: EventData,
    ) -> Self {
        Self {
            cx: Some(cx),
            gx,
            modal_context: None,
            event_data,
        }
    }

    /// Create a HandlerContext for modal handlers.
    pub fn for_modal<R: Send + Sync + 'static>(
        cx: &'a AppContext,
        gx: &'a GlobalContext,
        mx: &'a ModalContext<R>,
    ) -> Self {
        Self {
            cx: Some(cx),
            gx,
            modal_context: Some(mx),
            event_data: EventData::None,
        }
    }

    /// Create a HandlerContext for modal handlers with type-erased modal context.
    pub fn for_modal_any(
        cx: &'a AppContext,
        gx: &'a GlobalContext,
        mx: &'a (dyn std::any::Any + Send + Sync),
    ) -> Self {
        Self {
            cx: Some(cx),
            gx,
            modal_context: Some(mx),
            event_data: EventData::None,
        }
    }

    /// Create a HandlerContext for system handlers (no app context).
    pub fn for_system(gx: &'a GlobalContext) -> Self {
        Self {
            cx: None,
            gx,
            modal_context: None,
            event_data: EventData::None,
        }
    }

    /// Get the app context.
    ///
    /// # Panics
    ///
    /// Panics if called from a system handler. With compile-time checks
    /// in `#[system_impl]`, this should never happen in correctly written code.
    pub fn cx(&self) -> &AppContext {
        self.cx.expect("cx() called in system context (no AppContext available)")
    }

    /// Try to get the app context (returns None for systems).
    pub fn try_cx(&self) -> Option<&AppContext> {
        self.cx
    }

    /// Get the global context.
    pub fn gx(&self) -> &GlobalContext {
        self.gx
    }

    /// Get the modal context.
    ///
    /// # Panics
    ///
    /// Panics if called outside a modal context. With compile-time checks
    /// in `#[app_impl]` and `#[modal_impl]`, this should never happen in
    /// correctly written code.
    pub fn mx<R: Send + Sync + 'static>(&self) -> &ModalContext<R> {
        self.modal_context
            .expect("mx() called outside modal context")
            .downcast_ref::<ModalContext<R>>()
            .expect("ModalContext type mismatch")
    }

    /// Try to get the modal context (returns None if not in a modal).
    pub fn try_mx<R: Send + Sync + 'static>(&self) -> Option<&ModalContext<R>> {
        self.modal_context?.downcast_ref()
    }

    /// Check if this context has an app context.
    pub fn has_app_context(&self) -> bool {
        self.cx.is_some()
    }

    /// Check if this context is for a modal.
    pub fn is_modal(&self) -> bool {
        self.modal_context.is_some()
    }

    /// Get the event data.
    ///
    /// Returns the event-specific data passed when the handler was invoked.
    /// For example, `EventData::Change { text }` for input change handlers.
    /// Returns `EventData::None` if no event data was provided.
    pub fn event(&self) -> &EventData {
        &self.event_data
    }
}
