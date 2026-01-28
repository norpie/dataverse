//! Cursor state tracking.
//!
//! Tracks the current mouse cursor position across rendering loops.

use crate::event::Event;

/// Tracks the current mouse cursor position.
#[derive(Debug, Clone)]
#[derive(Default)]
pub struct CursorState {
    position: (u16, u16),
}


impl CursorState {
    /// Create a new cursor state with position at (0, 0).
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the current cursor position.
    pub fn position(&self) -> (u16, u16) {
        self.position
    }

    /// Process events and update cursor position.
    ///
    /// Captures `MouseMove` events to track position, passes all events through unchanged.
    pub fn process_events(&mut self, events: &[Event]) -> Vec<Event> {
        for event in events {
            if let Event::MouseMove { x, y } = event {
                self.position = (*x, *y);
            }
        }
        events.to_vec()
    }
}
