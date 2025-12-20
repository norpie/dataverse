//! Event handling for the List component.

use crate::components::events::{ComponentEvents, EventResult};
use crate::keybinds::{Key, KeyCombo};

use super::state::{List, ListItem, SelectionMode};

/// Event fired when an item is activated (click or Enter).
#[derive(Debug, Clone)]
pub struct ActivateEvent {
    /// The index of the activated item.
    pub index: usize,
}

/// Event fired when the selection changes.
#[derive(Debug, Clone)]
pub struct SelectionChangeEvent {
    /// All currently selected indices.
    pub selected: Vec<usize>,
    /// Indices that were added to the selection.
    pub added: Vec<usize>,
    /// Indices that were removed from the selection.
    pub removed: Vec<usize>,
}

/// Event fired when the cursor moves.
#[derive(Debug, Clone)]
pub struct CursorMoveEvent {
    /// Previous cursor position (None if no previous cursor).
    pub previous: Option<usize>,
    /// Current cursor position.
    pub current: usize,
}

/// Pending events to be dispatched after input handling.
#[derive(Debug, Clone, Default)]
pub struct ListEvents {
    pub activate: Option<ActivateEvent>,
    pub selection_change: Option<SelectionChangeEvent>,
    pub cursor_move: Option<CursorMoveEvent>,
}

impl<T: ListItem> List<T> {
    /// Handle a click at the given y-offset within the list viewport.
    /// Returns events that should be dispatched.
    pub fn handle_click(
        &self,
        y_in_viewport: u16,
        ctrl: bool,
        shift: bool,
    ) -> ListEvents {
        let mut events = ListEvents::default();

        let scroll_offset = self.scroll_offset();
        let item_height = T::HEIGHT;
        let absolute_y = scroll_offset + y_in_viewport;
        let index = (absolute_y / item_height) as usize;

        if index >= self.len() {
            return events;
        }

        // Move cursor
        let previous = self.set_cursor(index);
        if previous != Some(index) {
            events.cursor_move = Some(CursorMoveEvent {
                previous,
                current: index,
            });
        }

        // Handle selection based on modifiers
        match self.selection_mode() {
            SelectionMode::None => {
                // Just activate on click
                events.activate = Some(ActivateEvent { index });
            }
            SelectionMode::Single => {
                if ctrl {
                    // Toggle selection
                    let (added, removed) = self.toggle_select(index);
                    if !added.is_empty() || !removed.is_empty() {
                        events.selection_change = Some(SelectionChangeEvent {
                            selected: self.selected_indices(),
                            added,
                            removed,
                        });
                    }
                } else {
                    // Activate
                    events.activate = Some(ActivateEvent { index });
                }
            }
            SelectionMode::Multiple => {
                if shift {
                    // Range select
                    let (added, removed) = self.range_select(index, ctrl);
                    if !added.is_empty() || !removed.is_empty() {
                        events.selection_change = Some(SelectionChangeEvent {
                            selected: self.selected_indices(),
                            added,
                            removed,
                        });
                    }
                } else if ctrl {
                    // Toggle selection
                    let (added, removed) = self.toggle_select(index);
                    if !added.is_empty() || !removed.is_empty() {
                        events.selection_change = Some(SelectionChangeEvent {
                            selected: self.selected_indices(),
                            added,
                            removed,
                        });
                    }
                } else {
                    // Activate
                    events.activate = Some(ActivateEvent { index });
                }
            }
        }

        // Ensure cursor is visible
        self.scroll_to_cursor();

        events
    }

    /// Handle keyboard input. Returns events that should be dispatched.
    pub fn handle_key(&self, key: &KeyCombo) -> (EventResult, ListEvents) {
        let mut events = ListEvents::default();

        // Navigation keys
        match key.key {
            Key::Up | Key::Char('k') if !key.modifiers.ctrl && !key.modifiers.alt => {
                if let Some((prev, curr)) = self.cursor_up() {
                    events.cursor_move = Some(CursorMoveEvent {
                        previous: prev,
                        current: curr,
                    });
                    self.scroll_to_cursor();
                    return (EventResult::Consumed, events);
                }
            }
            Key::Down | Key::Char('j') if !key.modifiers.ctrl && !key.modifiers.alt => {
                if let Some((prev, curr)) = self.cursor_down() {
                    events.cursor_move = Some(CursorMoveEvent {
                        previous: prev,
                        current: curr,
                    });
                    self.scroll_to_cursor();
                    return (EventResult::Consumed, events);
                }
            }
            Key::Home | Key::Char('g') if !key.modifiers.ctrl && !key.modifiers.alt => {
                if let Some((prev, curr)) = self.cursor_first() {
                    events.cursor_move = Some(CursorMoveEvent {
                        previous: prev,
                        current: curr,
                    });
                    self.scroll_to_cursor();
                    return (EventResult::Consumed, events);
                }
            }
            Key::End | Key::Char('G') if !key.modifiers.ctrl && !key.modifiers.alt => {
                if let Some((prev, curr)) = self.cursor_last() {
                    events.cursor_move = Some(CursorMoveEvent {
                        previous: prev,
                        current: curr,
                    });
                    self.scroll_to_cursor();
                    return (EventResult::Consumed, events);
                }
            }
            Key::Enter if !key.modifiers.ctrl && !key.modifiers.alt => {
                // Activate current cursor
                if let Some(index) = self.cursor() {
                    events.activate = Some(ActivateEvent { index });
                    return (EventResult::Consumed, events);
                }
            }
            Key::Space if !key.modifiers.ctrl && !key.modifiers.alt => {
                // Toggle selection at cursor
                if let Some(index) = self.cursor() {
                    if self.selection_mode() != SelectionMode::None {
                        let (added, removed) = self.toggle_select(index);
                        if !added.is_empty() || !removed.is_empty() {
                            events.selection_change = Some(SelectionChangeEvent {
                                selected: self.selected_indices(),
                                added,
                                removed,
                            });
                        }
                        return (EventResult::Consumed, events);
                    }
                }
            }
            Key::Char('a') if key.modifiers.ctrl => {
                // Select all
                if self.selection_mode() == SelectionMode::Multiple {
                    let added = self.select_all();
                    if !added.is_empty() {
                        events.selection_change = Some(SelectionChangeEvent {
                            selected: self.selected_indices(),
                            added,
                            removed: vec![],
                        });
                    }
                    return (EventResult::Consumed, events);
                }
            }
            Key::Escape => {
                // Clear selection
                let removed = self.deselect_all();
                if !removed.is_empty() {
                    events.selection_change = Some(SelectionChangeEvent {
                        selected: vec![],
                        added: vec![],
                        removed,
                    });
                    return (EventResult::Consumed, events);
                }
            }
            Key::PageUp => {
                // Move cursor up by viewport
                let viewport_items = (self.viewport_height() / T::HEIGHT) as usize;
                if let Some(cursor) = self.cursor() {
                    let new_cursor = cursor.saturating_sub(viewport_items);
                    if new_cursor != cursor {
                        let previous = self.set_cursor(new_cursor);
                        events.cursor_move = Some(CursorMoveEvent {
                            previous,
                            current: new_cursor,
                        });
                        self.scroll_to_cursor();
                        return (EventResult::Consumed, events);
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
                        let previous = self.set_cursor(new_cursor);
                        events.cursor_move = Some(CursorMoveEvent {
                            previous,
                            current: new_cursor,
                        });
                        self.scroll_to_cursor();
                        return (EventResult::Consumed, events);
                    }
                }
            }
            _ => {}
        }

        (EventResult::Ignored, events)
    }
}

impl<T: ListItem> ComponentEvents for List<T> {
    fn on_key(&self, key: &KeyCombo) -> EventResult {
        // Basic key handling - events are handled separately via handle_key
        let (result, _events) = self.handle_key(key);
        result
    }

    fn on_click(&self, _x: u16, y: u16) -> EventResult {
        // Calculate y relative to viewport (assumes click is within list bounds)
        let _events = self.handle_click(y, false, false);
        EventResult::Consumed
    }
}
