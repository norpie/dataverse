//! Widget event handling types and traits.
//!
//! This module defines the core types for widget-based event handling,
//! allowing each widget to handle its own events while keeping the
//! event loop as a thin dispatcher.
//!
//! Components push events to the event queue via `AppContext::push_event()`.
//! The event loop then drains the queue and dispatches appropriate handlers.

use crate::context::AppContext;
use crate::events::{Modifiers, ScrollDirection};
use crate::keybinds::KeyCombo;

// =============================================================================
// Widget Event Types
// =============================================================================

/// Identifies which handler to call for a widget event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidgetEventKind {
    /// Item/row/node activated (Enter, double-click)
    Activate,
    /// Cursor moved to new position
    CursorMove,
    /// Selection changed
    SelectionChange,
    /// Tree node expanded
    Expand,
    /// Tree node collapsed
    Collapse,
    /// Table column sorted
    Sort,
    /// Value changed (input text, checkbox state, radio selection)
    Change,
}

/// A widget event to be dispatched.
///
/// Components push these events via `AppContext::push_event()`.
/// The event loop drains and dispatches them after each user interaction.
#[derive(Debug, Clone)]
pub struct WidgetEvent {
    /// Which kind of event
    pub kind: WidgetEventKind,
    /// Widget ID that triggered the event
    pub widget_id: String,
}

impl WidgetEvent {
    /// Create a new widget event.
    pub fn new(kind: WidgetEventKind, widget_id: impl Into<String>) -> Self {
        Self {
            kind,
            widget_id: widget_id.into(),
        }
    }
}

// =============================================================================
// Event Result
// =============================================================================

/// Result of handling an event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventResult {
    /// Event was ignored, try other handlers.
    Ignored,
    /// Event was consumed, stop propagation.
    Consumed,
    /// Event started a drag operation on this widget.
    StartDrag,
}

impl EventResult {
    /// Check if the event was handled (consumed or started drag).
    pub fn is_handled(&self) -> bool {
        !matches!(self, EventResult::Ignored)
    }
}

/// Trait for widgets that can handle events.
///
/// Components implement this trait to handle mouse and keyboard events.
/// The event loop dispatches events to widgets through these methods,
/// allowing widget-specific behavior to be encapsulated within the widget.
///
/// Components should set any relevant context data via `AppContext` (e.g.,
/// `cx.set_list_cursor()`, `cx.set_input_text()`) before returning. The event
/// loop will then dispatch appropriate handlers based on what context was set.
///
/// # Default Implementations
///
/// All methods have default implementations that return `EventResult::Ignored`,
/// so widgets only need to implement the events they care about.
pub trait WidgetEvents {
    /// Handle a click event at the given position.
    ///
    /// Called when the user clicks within the widget's bounds.
    /// Return `EventResult::StartDrag` to begin a drag operation.
    fn on_click(&self, _x: u16, _y: u16, _cx: &AppContext) -> EventResult {
        EventResult::Ignored
    }

    /// Handle a hover event at the given position.
    ///
    /// Called when the mouse moves within the widget's bounds.
    fn on_hover(&self, _x: u16, _y: u16, _cx: &AppContext) -> EventResult {
        EventResult::Ignored
    }

    /// Handle a scroll event.
    ///
    /// Called when the user scrolls (mouse wheel) within the widget's bounds.
    fn on_scroll(
        &self,
        _direction: ScrollDirection,
        _amount: u16,
        _cx: &AppContext,
    ) -> EventResult {
        EventResult::Ignored
    }

    /// Handle ongoing drag movement.
    ///
    /// Called when the user drags after a `StartDrag` result from `on_click`.
    /// The widget should track its own drag state internally.
    fn on_drag(&self, _x: u16, _y: u16, _modifiers: Modifiers, _cx: &AppContext) -> EventResult {
        EventResult::Ignored
    }

    /// Handle drag release.
    ///
    /// Called when the user releases the mouse button after dragging.
    /// The widget should clear any internal drag state.
    fn on_release(&self, _cx: &AppContext) -> EventResult {
        EventResult::Ignored
    }

    /// Handle a key event when this widget is focused.
    ///
    /// Called when the user presses a key while this widget has focus.
    /// Return `EventResult::Consumed` to prevent the key from being
    /// processed as a keybind.
    fn on_key(&self, _key: &KeyCombo, _cx: &AppContext) -> EventResult {
        EventResult::Ignored
    }
}
