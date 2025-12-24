//! Event loop state management.

use std::sync::Arc;
use std::time::Instant;

use crate::context::Toast;
use crate::input::focus::FocusState;
use crate::input::keybinds::Keybinds;
use crate::layers::overlay::ActiveOverlay;
use crate::styling::theme::Theme;
use crate::system::AnySystem;

use super::input::InputState;
use super::modal::ModalStackEntry;

/// Bundled state for the event loop.
///
/// This struct consolidates all the mutable state that the event loop
/// needs to track across iterations.
pub struct EventLoopState {
    /// App-level focus state (used when no modal is open).
    pub app_focus_state: FocusState,

    /// App-level input state for keybind sequence tracking.
    pub app_input_state: InputState,

    /// System-level input state for keybind sequence tracking.
    pub system_input_state: InputState,

    /// Merged keybinds from all registered systems.
    pub system_keybinds: Keybinds,

    /// Registered system instances.
    pub systems: Vec<Box<dyn AnySystem>>,

    /// Stack of open modals (each with its own focus/input state).
    pub modal_stack: Vec<ModalStackEntry>,

    /// Active toasts with their expiration times.
    pub active_toasts: Vec<(Toast, Instant)>,

    /// Current theme.
    pub current_theme: Arc<dyn Theme>,

    /// Widget currently being dragged (for scrollbar drag).
    pub drag_widget_id: Option<String>,

    /// Active overlays from the last render (for click-outside detection).
    pub active_overlays: Vec<ActiveOverlay>,

    /// Last toast count (for dirty tracking).
    pub last_toast_count: usize,

    /// Last focused element ID (for dirty tracking).
    pub last_focused_id: Option<String>,

    /// Flag indicating an event was dispatched (for render triggering).
    pub event_dispatched: bool,
}

impl EventLoopState {
    /// Create a new event loop state with the given initial theme and systems.
    pub fn new(theme: Arc<dyn Theme>, systems: Vec<Box<dyn AnySystem>>) -> Self {
        // Merge keybinds from all systems
        let mut system_keybinds = Keybinds::new();
        for system in &systems {
            system_keybinds.merge(system.keybinds());
        }

        Self {
            app_focus_state: FocusState::new(),
            app_input_state: InputState::new(),
            system_input_state: InputState::new(),
            system_keybinds,
            systems,
            modal_stack: Vec::new(),
            active_toasts: Vec::new(),
            current_theme: theme,
            drag_widget_id: None,
            active_overlays: Vec::new(),
            last_toast_count: 0,
            last_focused_id: None,
            event_dispatched: false,
        }
    }

    /// Check if we're in modal mode.
    pub fn in_modal(&self) -> bool {
        !self.modal_stack.is_empty()
    }

    /// Get the active focus state (modal or app).
    pub fn focus_state(&self) -> &FocusState {
        if let Some(entry) = self.modal_stack.last() {
            &entry.focus_state
        } else {
            &self.app_focus_state
        }
    }

    /// Get the active focus state mutably.
    pub fn focus_state_mut(&mut self) -> &mut FocusState {
        if let Some(entry) = self.modal_stack.last_mut() {
            &mut entry.focus_state
        } else {
            &mut self.app_focus_state
        }
    }

    /// Get the current focused element ID.
    pub fn focused_id(&self) -> Option<String> {
        self.focus_state().current().map(|f| f.0.clone())
    }
}
