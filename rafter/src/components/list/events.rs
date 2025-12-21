//! Event handling for the List component.

use crate::components::events::{ComponentEvents, EventResult};
use crate::components::scrollbar::{
    ScrollbarState, handle_scrollbar_click, handle_scrollbar_drag, handle_scrollbar_release,
};
use crate::components::traits::SelectableComponent;
use crate::context::AppContext;
use crate::events::{Modifiers, ScrollDirection};
use crate::keybinds::KeyCombo;

use super::SelectionMode;
use super::item::ListItem;
use super::state::List;

impl<T: ListItem> ComponentEvents for List<T> {
    fn on_key(&self, key: &KeyCombo, cx: &AppContext) -> EventResult {
        // Try navigation keys first (Up, Down, Home, End, PageUp, PageDown)
        if let Some(result) = self.handle_navigation_key(key, cx) {
            return result;
        }

        // Try selection keys (Space, Ctrl+A, Escape, Enter)
        if let Some(result) = self.handle_selection_key(key, cx) {
            return result;
        }

        EventResult::Ignored
    }

    fn on_click(&self, x: u16, y: u16, cx: &AppContext) -> EventResult {
        // Delegate scrollbar click handling to shared helper
        if let Some(result) = handle_scrollbar_click(self, x, y, cx) {
            return result;
        }

        // If not on scrollbar, return Ignored - let the event loop handle
        // the click with modifiers via on_click_with_modifiers
        EventResult::Ignored
    }

    fn on_hover(&self, _x: u16, y: u16, cx: &AppContext) -> EventResult {
        self.handle_hover(y, cx)
    }

    fn on_scroll(&self, direction: ScrollDirection, amount: u16, _cx: &AppContext) -> EventResult {
        let amount = amount as i16;
        match direction {
            ScrollDirection::Up => ScrollbarState::scroll_by(self, 0, -amount),
            ScrollDirection::Down => ScrollbarState::scroll_by(self, 0, amount),
            ScrollDirection::Left | ScrollDirection::Right => {
                // List only scrolls vertically
                return EventResult::Ignored;
            }
        }
        EventResult::Consumed
    }

    fn on_drag(&self, x: u16, y: u16, modifiers: Modifiers, cx: &AppContext) -> EventResult {
        handle_scrollbar_drag(self, x, y, modifiers, cx)
    }

    fn on_release(&self, cx: &AppContext) -> EventResult {
        handle_scrollbar_release(self, cx)
    }
}

impl<T: ListItem> List<T> {
    /// Handle click with modifier keys (Ctrl, Shift).
    /// This is called by the event loop when it has access to modifier state.
    pub fn on_click_with_modifiers(
        &self,
        y_in_viewport: u16,
        ctrl: bool,
        shift: bool,
        cx: &AppContext,
    ) -> EventResult {
        let Some(index) = self.index_from_viewport_y(y_in_viewport) else {
            return EventResult::Ignored;
        };

        // Move cursor
        self.handle_cursor_move(index, cx);

        // Handle selection based on modifiers
        match self.selection_mode() {
            SelectionMode::None => {
                // Just activate on click
                self.push_activate_event(cx);
            }
            SelectionMode::Single => {
                if ctrl {
                    // Toggle selection
                    let (added, removed) = self.toggle_select(index);
                    self.push_selection_event(&added, &removed, cx);
                } else {
                    // Activate
                    self.push_activate_event(cx);
                }
            }
            SelectionMode::Multiple => {
                if shift {
                    // Range select
                    let (added, removed) = self.range_select(index, ctrl);
                    self.push_selection_event(&added, &removed, cx);
                } else if ctrl {
                    // Toggle selection
                    let (added, removed) = self.toggle_select(index);
                    self.push_selection_event(&added, &removed, cx);
                } else {
                    // Activate
                    self.push_activate_event(cx);
                }
            }
        }

        // Ensure cursor is visible
        self.scroll_to_cursor();

        EventResult::Consumed
    }
}
