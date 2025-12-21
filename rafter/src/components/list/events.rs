//! Event handling for the List component.

use crate::components::events::{ComponentEvents, EventResult};
use crate::components::scrollbar::{
    handle_scrollbar_click, handle_scrollbar_drag, handle_scrollbar_release, ScrollbarState,
};
use crate::context::AppContext;
use crate::events::{Modifiers, ScrollDirection};
use crate::keybinds::{Key, KeyCombo};

use super::state::{List, ListItem, SelectionMode};

impl<T: ListItem> List<T> {
    /// Calculate the list item index from a y-offset within the viewport.
    fn index_from_viewport_y(&self, y_in_viewport: u16) -> Option<usize> {
        let scroll_offset = self.scroll_offset();
        let item_height = T::HEIGHT;
        let absolute_y = scroll_offset + y_in_viewport;
        let index = (absolute_y / item_height) as usize;

        if index < self.len() {
            Some(index)
        } else {
            None
        }
    }

    /// Handle cursor movement, setting context and returning true if cursor changed.
    fn handle_cursor_move(&self, new_cursor: usize, cx: &AppContext) -> bool {
        let previous = self.set_cursor(new_cursor);
        if previous != Some(new_cursor) {
            cx.set_list_cursor(new_cursor);
            true
        } else {
            false
        }
    }

    /// Handle activation, setting context.
    fn handle_activate(&self, index: usize, cx: &AppContext) {
        cx.set_list_activated_index(index);
    }

    /// Handle selection change, setting context if selection changed.
    fn handle_selection_change(&self, added: Vec<usize>, removed: Vec<usize>, cx: &AppContext) {
        if !added.is_empty() || !removed.is_empty() {
            cx.set_list_selected_indices(self.selected_indices());
        }
    }
}

impl<T: ListItem> ComponentEvents for List<T> {
    fn on_key(&self, key: &KeyCombo, cx: &AppContext) -> EventResult {
        // Navigation keys
        match key.key {
            Key::Up if !key.modifiers.ctrl && !key.modifiers.alt => {
                if let Some((prev, curr)) = self.cursor_up() {
                    cx.set_list_cursor(curr);
                    let _ = prev; // Suppress unused warning
                    self.scroll_to_cursor();
                    return EventResult::Consumed;
                }
            }
            Key::Down if !key.modifiers.ctrl && !key.modifiers.alt => {
                if let Some((prev, curr)) = self.cursor_down() {
                    cx.set_list_cursor(curr);
                    let _ = prev;
                    self.scroll_to_cursor();
                    return EventResult::Consumed;
                }
            }
            Key::Home if !key.modifiers.ctrl && !key.modifiers.alt => {
                if let Some((prev, curr)) = self.cursor_first() {
                    cx.set_list_cursor(curr);
                    let _ = prev;
                    self.scroll_to_cursor();
                    return EventResult::Consumed;
                }
            }
            Key::End if !key.modifiers.ctrl && !key.modifiers.alt => {
                if let Some((prev, curr)) = self.cursor_last() {
                    cx.set_list_cursor(curr);
                    let _ = prev;
                    self.scroll_to_cursor();
                    return EventResult::Consumed;
                }
            }
            Key::Enter if !key.modifiers.ctrl && !key.modifiers.alt => {
                // Activate current cursor
                if let Some(index) = self.cursor() {
                    cx.set_list_activated_index(index);
                    return EventResult::Consumed;
                }
            }
            Key::Space if !key.modifiers.ctrl && !key.modifiers.alt => {
                // Toggle selection at cursor
                if let Some(index) = self.cursor()
                    && self.selection_mode() != SelectionMode::None
                {
                    let (added, removed) = self.toggle_select(index);
                    self.handle_selection_change(added, removed, cx);
                    return EventResult::Consumed;
                }
            }
            Key::Char('a') if key.modifiers.ctrl => {
                // Select all
                if self.selection_mode() == SelectionMode::Multiple {
                    let added = self.select_all();
                    self.handle_selection_change(added, vec![], cx);
                    return EventResult::Consumed;
                }
            }
            Key::Escape => {
                // Clear selection
                let removed = self.deselect_all();
                if !removed.is_empty() {
                    self.handle_selection_change(vec![], removed, cx);
                    return EventResult::Consumed;
                }
            }
            Key::PageUp => {
                // Move cursor up by viewport
                let viewport_items = (self.viewport_height() / T::HEIGHT) as usize;
                if let Some(cursor) = self.cursor() {
                    let new_cursor = cursor.saturating_sub(viewport_items);
                    if new_cursor != cursor {
                        self.set_cursor(new_cursor);
                        cx.set_list_cursor(new_cursor);
                        self.scroll_to_cursor();
                        return EventResult::Consumed;
                    }
                }
            }
            Key::PageDown => {
                // Move cursor down by viewport
                let viewport_items = (self.viewport_height() / T::HEIGHT) as usize;
                if let Some(cursor) = self.cursor() {
                    let max_index = self.len().saturating_sub(1);
                    let new_cursor = (cursor + viewport_items).min(max_index);
                    if new_cursor != cursor {
                        self.set_cursor(new_cursor);
                        cx.set_list_cursor(new_cursor);
                        self.scroll_to_cursor();
                        return EventResult::Consumed;
                    }
                }
            }
            _ => {}
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
        // Move cursor on hover (y is relative to list content area)
        if let Some(index) = self.index_from_viewport_y(y)
            && self.handle_cursor_move(index, cx)
        {
            return EventResult::Consumed;
        }
        EventResult::Ignored
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
                self.handle_activate(index, cx);
            }
            SelectionMode::Single => {
                if ctrl {
                    // Toggle selection
                    let (added, removed) = self.toggle_select(index);
                    self.handle_selection_change(added, removed, cx);
                } else {
                    // Activate
                    self.handle_activate(index, cx);
                }
            }
            SelectionMode::Multiple => {
                if shift {
                    // Range select
                    let (added, removed) = self.range_select(index, ctrl);
                    self.handle_selection_change(added, removed, cx);
                } else if ctrl {
                    // Toggle selection
                    let (added, removed) = self.toggle_select(index);
                    self.handle_selection_change(added, removed, cx);
                } else {
                    // Activate
                    self.handle_activate(index, cx);
                }
            }
        }

        // Ensure cursor is visible
        self.scroll_to_cursor();

        EventResult::Consumed
    }
}
