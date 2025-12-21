//! Event handling for the Tree component.

use crate::components::events::{ComponentEvent, ComponentEventKind, ComponentEvents, EventResult};
use crate::components::scrollbar::{
    ScrollbarState, handle_scrollbar_click, handle_scrollbar_drag, handle_scrollbar_release,
};
use crate::components::selection::SelectionMode;
use crate::context::AppContext;
use crate::events::{Modifiers, ScrollDirection};
use crate::keybinds::{Key, KeyCombo};

use super::item::TreeItem;
use super::state::Tree;

impl<T: TreeItem> Tree<T> {
    /// Calculate the visible node index from a y-offset within the viewport.
    fn index_from_viewport_y(&self, y_in_viewport: u16) -> Option<usize> {
        let scroll_offset = self.scroll_offset();
        let item_height = T::HEIGHT;
        let absolute_y = scroll_offset + y_in_viewport;
        let index = (absolute_y / item_height) as usize;

        if index < self.visible_len() {
            Some(index)
        } else {
            None
        }
    }

    /// Handle cursor movement, pushing event if cursor changed.
    fn handle_cursor_move(&self, new_cursor: usize, cx: &AppContext) -> bool {
        let previous = self.set_cursor(new_cursor);
        if previous != Some(new_cursor) {
            if let Some(id) = self.cursor_id() {
                cx.set_cursor(id, None);
                cx.push_event(ComponentEvent::new(
                    ComponentEventKind::CursorMove,
                    self.id_string(),
                ));
            }
            true
        } else {
            false
        }
    }

    /// Handle activation, pushing event.
    fn handle_activate(&self, cx: &AppContext) {
        if let Some(id) = self.cursor_id() {
            cx.set_activated(id, None);
            cx.push_event(ComponentEvent::new(
                ComponentEventKind::Activate,
                self.id_string(),
            ));
        }
    }

    /// Handle selection change, pushing event if selection changed.
    fn handle_selection_change(&self, added: Vec<String>, removed: Vec<String>, cx: &AppContext) {
        if !added.is_empty() || !removed.is_empty() {
            cx.set_selected(self.selected_ids());
            cx.push_event(ComponentEvent::new(
                ComponentEventKind::SelectionChange,
                self.id_string(),
            ));
        }
    }

    /// Handle expand event, pushing event.
    fn handle_expand(&self, node_id: &str, cx: &AppContext) {
        cx.set_expanded(node_id.to_string());
        cx.push_event(ComponentEvent::new(
            ComponentEventKind::Expand,
            self.id_string(),
        ));
    }

    /// Handle collapse event, pushing event.
    fn handle_collapse(&self, node_id: &str, cx: &AppContext) {
        cx.set_collapsed(node_id.to_string());
        cx.push_event(ComponentEvent::new(
            ComponentEventKind::Collapse,
            self.id_string(),
        ));
    }
}

impl<T: TreeItem> ComponentEvents for Tree<T> {
    fn on_key(&self, key: &KeyCombo, cx: &AppContext) -> EventResult {
        match key.key {
            // Navigation
            Key::Up if !key.modifiers.ctrl && !key.modifiers.alt => {
                if let Some((_, _)) = self.cursor_up() {
                    if let Some(id) = self.cursor_id() {
                        cx.set_cursor(id, None);
                        cx.push_event(ComponentEvent::new(
                            ComponentEventKind::CursorMove,
                            self.id_string(),
                        ));
                    }
                    self.scroll_to_cursor();
                    return EventResult::Consumed;
                }
            }
            Key::Down if !key.modifiers.ctrl && !key.modifiers.alt => {
                if let Some((_, _)) = self.cursor_down() {
                    if let Some(id) = self.cursor_id() {
                        cx.set_cursor(id, None);
                        cx.push_event(ComponentEvent::new(
                            ComponentEventKind::CursorMove,
                            self.id_string(),
                        ));
                    }
                    self.scroll_to_cursor();
                    return EventResult::Consumed;
                }
            }
            Key::Home if !key.modifiers.ctrl && !key.modifiers.alt => {
                if let Some((_, _)) = self.cursor_first() {
                    if let Some(id) = self.cursor_id() {
                        cx.set_cursor(id, None);
                        cx.push_event(ComponentEvent::new(
                            ComponentEventKind::CursorMove,
                            self.id_string(),
                        ));
                    }
                    self.scroll_to_cursor();
                    return EventResult::Consumed;
                }
            }
            Key::End if !key.modifiers.ctrl && !key.modifiers.alt => {
                if let Some((_, _)) = self.cursor_last() {
                    if let Some(id) = self.cursor_id() {
                        cx.set_cursor(id, None);
                        cx.push_event(ComponentEvent::new(
                            ComponentEventKind::CursorMove,
                            self.id_string(),
                        ));
                    }
                    self.scroll_to_cursor();
                    return EventResult::Consumed;
                }
            }
            Key::PageUp => {
                let viewport_items = (self.viewport_height() / T::HEIGHT) as usize;
                if let Some(cursor) = self.cursor() {
                    let new_cursor = cursor.saturating_sub(viewport_items);
                    if new_cursor != cursor {
                        self.set_cursor(new_cursor);
                        if let Some(id) = self.cursor_id() {
                            cx.set_cursor(id, None);
                            cx.push_event(ComponentEvent::new(
                                ComponentEventKind::CursorMove,
                                self.id_string(),
                            ));
                        }
                        self.scroll_to_cursor();
                        return EventResult::Consumed;
                    }
                }
            }
            Key::PageDown => {
                let viewport_items = (self.viewport_height() / T::HEIGHT) as usize;
                if let Some(cursor) = self.cursor() {
                    let max_index = self.visible_len().saturating_sub(1);
                    let new_cursor = (cursor + viewport_items).min(max_index);
                    if new_cursor != cursor {
                        self.set_cursor(new_cursor);
                        if let Some(id) = self.cursor_id() {
                            cx.set_cursor(id, None);
                            cx.push_event(ComponentEvent::new(
                                ComponentEventKind::CursorMove,
                                self.id_string(),
                            ));
                        }
                        self.scroll_to_cursor();
                        return EventResult::Consumed;
                    }
                }
            }

            // Expand/Collapse with Left/Right
            Key::Left if !key.modifiers.ctrl && !key.modifiers.alt => {
                if let Some(node) = self.visible_node(self.cursor().unwrap_or(0)) {
                    if node.is_expanded {
                        // Collapse the current node
                        let id = node.item.id();
                        self.collapse(&id);
                        self.handle_collapse(&id, cx);
                        return EventResult::Consumed;
                    } else {
                        // Move to parent
                        if let Some((_, _)) = self.cursor_to_parent() {
                            if let Some(id) = self.cursor_id() {
                                cx.set_cursor(id, None);
                                cx.push_event(ComponentEvent::new(
                                    ComponentEventKind::CursorMove,
                                    self.id_string(),
                                ));
                            }
                            self.scroll_to_cursor();
                            return EventResult::Consumed;
                        }
                    }
                }
            }
            Key::Right if !key.modifiers.ctrl && !key.modifiers.alt => {
                if let Some(node) = self.visible_node(self.cursor().unwrap_or(0)) {
                    if node.has_children && !node.is_expanded {
                        // Expand the current node
                        let id = node.item.id();
                        self.expand(&id);
                        self.handle_expand(&id, cx);
                        return EventResult::Consumed;
                    } else if node.is_expanded {
                        // Move to first child
                        if let Some((_, _)) = self.cursor_to_first_child() {
                            if let Some(id) = self.cursor_id() {
                                cx.set_cursor(id, None);
                                cx.push_event(ComponentEvent::new(
                                    ComponentEventKind::CursorMove,
                                    self.id_string(),
                                ));
                            }
                            self.scroll_to_cursor();
                            return EventResult::Consumed;
                        }
                    }
                }
            }

            // Activation
            Key::Enter if !key.modifiers.ctrl && !key.modifiers.alt => {
                if self.cursor().is_some() {
                    self.handle_activate(cx);
                    return EventResult::Consumed;
                }
            }

            // Selection
            Key::Space if !key.modifiers.ctrl && !key.modifiers.alt => {
                if let Some(id) = self.cursor_id()
                    && self.selection_mode() != SelectionMode::None
                {
                    let (added, removed) = self.toggle_select(&id);
                    self.handle_selection_change(added, removed, cx);
                    return EventResult::Consumed;
                }
            }
            Key::Char('a') if key.modifiers.ctrl => {
                if self.selection_mode() == SelectionMode::Multiple {
                    let added = self.select_all();
                    self.handle_selection_change(added, vec![], cx);
                    return EventResult::Consumed;
                }
            }
            Key::Escape => {
                let removed = self.deselect_all();
                if !removed.is_empty() {
                    self.handle_selection_change(vec![], removed, cx);
                    return EventResult::Consumed;
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

impl<T: TreeItem> Tree<T> {
    /// Handle click with modifier keys (Ctrl, Shift).
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

        let Some(node) = self.visible_node(index) else {
            return EventResult::Ignored;
        };
        let id = node.item.id();

        // Handle selection based on modifiers
        match self.selection_mode() {
            SelectionMode::None => {
                self.handle_activate(cx);
            }
            SelectionMode::Single => {
                if ctrl {
                    let (added, removed) = self.toggle_select(&id);
                    self.handle_selection_change(added, removed, cx);
                } else {
                    self.handle_activate(cx);
                }
            }
            SelectionMode::Multiple => {
                if shift {
                    let (added, removed) = self.range_select(&id, ctrl);
                    self.handle_selection_change(added, removed, cx);
                } else if ctrl {
                    let (added, removed) = self.toggle_select(&id);
                    self.handle_selection_change(added, removed, cx);
                } else {
                    self.handle_activate(cx);
                }
            }
        }

        self.scroll_to_cursor();
        EventResult::Consumed
    }
}
