//! Component event handling types and traits.
//!
//! This module defines the core types for component-based event handling,
//! allowing each component to handle its own events while keeping the
//! event loop as a thin dispatcher.

use crate::events::ScrollDirection;
use crate::keybinds::KeyCombo;

/// Result of handling an event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventResult {
    /// Event was ignored, try other handlers.
    Ignored,
    /// Event was consumed, stop propagation.
    Consumed,
    /// Event started a drag operation on this component.
    StartDrag,
}

impl EventResult {
    /// Check if the event was handled (consumed or started drag).
    pub fn is_handled(&self) -> bool {
        !matches!(self, EventResult::Ignored)
    }
}

/// Trait for components that can handle events.
///
/// Components implement this trait to handle mouse and keyboard events.
/// The event loop dispatches events to components through these methods,
/// allowing component-specific behavior to be encapsulated within the component.
///
/// # Default Implementations
///
/// All methods have default implementations that return `EventResult::Ignored`,
/// so components only need to implement the events they care about.
pub trait ComponentEvents {
    /// Handle a click event at the given position.
    ///
    /// Called when the user clicks within the component's bounds.
    /// Return `EventResult::StartDrag` to begin a drag operation.
    fn on_click(&self, _x: u16, _y: u16) -> EventResult {
        EventResult::Ignored
    }

    /// Handle a scroll event.
    ///
    /// Called when the user scrolls (mouse wheel) within the component's bounds.
    fn on_scroll(&self, _direction: ScrollDirection, _amount: u16) -> EventResult {
        EventResult::Ignored
    }

    /// Handle ongoing drag movement.
    ///
    /// Called when the user drags after a `StartDrag` result from `on_click`.
    /// The component should track its own drag state internally.
    fn on_drag(&self, _x: u16, _y: u16) -> EventResult {
        EventResult::Ignored
    }

    /// Handle drag release.
    ///
    /// Called when the user releases the mouse button after dragging.
    /// The component should clear any internal drag state.
    fn on_release(&self) -> EventResult {
        EventResult::Ignored
    }

    /// Handle a key event when this component is focused.
    ///
    /// Called when the user presses a key while this component has focus.
    /// Return `EventResult::Consumed` to prevent the key from being
    /// processed as a keybind.
    fn on_key(&self, _key: &KeyCombo) -> EventResult {
        EventResult::Ignored
    }
}
