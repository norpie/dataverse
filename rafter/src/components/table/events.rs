//! Event handling for the Table component.

use crate::components::events::{ComponentEvents, EventResult};
use crate::components::scrollbar::{
    ScrollbarState, handle_scrollbar_click, handle_scrollbar_drag, handle_scrollbar_release,
};
use crate::components::selection::SelectionMode;
use crate::context::AppContext;
use crate::events::{Modifiers, ScrollDirection};
use crate::keybinds::{Key, KeyCombo};

use super::item::TableRow;
use super::state::Table;

/// Horizontal scroll amount per key press (in terminal columns).
const HORIZONTAL_SCROLL_AMOUNT: i16 = 10;

impl<T: TableRow> Table<T> {
    /// Calculate the row index from a y-offset within the data viewport.
    /// The y offset is relative to the table's origin (0 = header row).
    fn index_from_viewport_y(&self, y_in_viewport: u16) -> Option<usize> {
        // First row (y=0) is the header, so data starts at y=1
        if y_in_viewport == 0 {
            return None; // Click on header
        }
        let y_in_data = y_in_viewport.saturating_sub(1);
        let scroll_offset = self.scroll_offset_y();
        let row_height = T::HEIGHT;
        let absolute_y = scroll_offset + y_in_data;
        let index = (absolute_y / row_height) as usize;

        if index < self.len() {
            Some(index)
        } else {
            None
        }
    }

    /// Calculate which column was clicked based on x position.
    fn column_from_x(&self, x: u16) -> Option<usize> {
        let scroll_x = self.scroll_offset_x();
        let absolute_x = scroll_x + x;

        self.inner.read().ok().and_then(|g| {
            if g.columns.is_empty() {
                return None;
            }
            // Find which column this x falls into
            let mut col_x = 0u16;
            for (i, col) in g.columns.iter().enumerate() {
                if absolute_x >= col_x && absolute_x < col_x + col.width {
                    return Some(i);
                }
                col_x += col.width;
            }
            None
        })
    }

    /// Handle cursor movement, setting context and returning true if cursor changed.
    fn handle_cursor_move(&self, new_cursor: usize, cx: &AppContext) -> bool {
        let previous = self.set_cursor(new_cursor);
        if previous != Some(new_cursor) {
            if let Some(id) = self.cursor_id() {
                cx.set_table_cursor_id(id);
            }
            true
        } else {
            false
        }
    }

    /// Handle activation, setting context.
    fn handle_activate(&self, cx: &AppContext) {
        if let Some(id) = self.cursor_id() {
            cx.set_table_activated_id(id);
        }
    }

    /// Handle selection change, setting context if selection changed.
    fn handle_selection_change(&self, added: Vec<String>, removed: Vec<String>, cx: &AppContext) {
        if !added.is_empty() || !removed.is_empty() {
            cx.set_table_selected_ids(self.selected_ids());
        }
    }

    /// Handle header click for sorting.
    pub fn on_header_click(&self, x: u16, cx: &AppContext) -> EventResult {
        let Some(col_idx) = self.column_from_x(x) else {
            return EventResult::Ignored;
        };

        // Check if column is sortable
        let sortable = self
            .inner
            .read()
            .map(|g| g.columns.get(col_idx).is_some_and(|c| c.sortable))
            .unwrap_or(false);

        if !sortable {
            return EventResult::Ignored;
        }

        // Toggle sort
        if let Some((col, asc)) = self.toggle_sort(col_idx) {
            cx.set_table_sorted_column(col, asc);
            return EventResult::Consumed;
        }

        EventResult::Ignored
    }

    /// Handle click on data row with modifiers.
    pub fn on_row_click(
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

        let Some(row) = self.row(index) else {
            return EventResult::Ignored;
        };
        let id = row.id();

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

impl<T: TableRow> ComponentEvents for Table<T> {
    fn on_key(&self, key: &KeyCombo, cx: &AppContext) -> EventResult {
        match key.key {
            // Vertical Navigation
            Key::Up if !key.modifiers.ctrl && !key.modifiers.alt => {
                if let Some((_, curr)) = self.cursor_up() {
                    if let Some(id) = self.cursor_id() {
                        cx.set_table_cursor_id(id);
                    }
                    let _ = curr;
                    self.scroll_to_cursor();
                    return EventResult::Consumed;
                }
            }
            Key::Down if !key.modifiers.ctrl && !key.modifiers.alt => {
                if let Some((_, curr)) = self.cursor_down() {
                    if let Some(id) = self.cursor_id() {
                        cx.set_table_cursor_id(id);
                    }
                    let _ = curr;
                    self.scroll_to_cursor();
                    return EventResult::Consumed;
                }
            }
            Key::Home if !key.modifiers.ctrl && !key.modifiers.alt => {
                if let Some((_, _)) = self.cursor_first() {
                    if let Some(id) = self.cursor_id() {
                        cx.set_table_cursor_id(id);
                    }
                    self.scroll_to_cursor();
                    return EventResult::Consumed;
                }
            }
            Key::End if !key.modifiers.ctrl && !key.modifiers.alt => {
                if let Some((_, _)) = self.cursor_last() {
                    if let Some(id) = self.cursor_id() {
                        cx.set_table_cursor_id(id);
                    }
                    self.scroll_to_cursor();
                    return EventResult::Consumed;
                }
            }
            Key::PageUp => {
                let data_viewport = self.data_viewport_height();
                let viewport_rows = (data_viewport / T::HEIGHT) as usize;
                if let Some(cursor) = self.cursor() {
                    let new_cursor = cursor.saturating_sub(viewport_rows);
                    if new_cursor != cursor {
                        self.set_cursor(new_cursor);
                        if let Some(id) = self.cursor_id() {
                            cx.set_table_cursor_id(id);
                        }
                        self.scroll_to_cursor();
                        return EventResult::Consumed;
                    }
                }
            }
            Key::PageDown => {
                let data_viewport = self.data_viewport_height();
                let viewport_rows = (data_viewport / T::HEIGHT) as usize;
                if let Some(cursor) = self.cursor() {
                    let max_index = self.len().saturating_sub(1);
                    let new_cursor = (cursor + viewport_rows).min(max_index);
                    if new_cursor != cursor {
                        self.set_cursor(new_cursor);
                        if let Some(id) = self.cursor_id() {
                            cx.set_table_cursor_id(id);
                        }
                        self.scroll_to_cursor();
                        return EventResult::Consumed;
                    }
                }
            }

            // Horizontal Scrolling with Left/Right
            Key::Left if !key.modifiers.ctrl && !key.modifiers.alt => {
                if self.needs_horizontal_scrollbar() {
                    self.scroll_x_by(-HORIZONTAL_SCROLL_AMOUNT);
                    return EventResult::Consumed;
                }
            }
            Key::Right if !key.modifiers.ctrl && !key.modifiers.alt => {
                if self.needs_horizontal_scrollbar() {
                    self.scroll_x_by(HORIZONTAL_SCROLL_AMOUNT);
                    return EventResult::Consumed;
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
        // First check for scrollbar clicks
        if let Some(result) = handle_scrollbar_click(self, x, y, cx) {
            return result;
        }

        // Check if click is on header row (y == 0)
        if y == 0 {
            return self.on_header_click(x, cx);
        }

        // Click on data row - return Ignored, let the event loop handle
        // the click with modifiers via on_row_click
        EventResult::Ignored
    }

    fn on_hover(&self, _x: u16, y: u16, cx: &AppContext) -> EventResult {
        // Only hover on data rows, not header
        if y == 0 {
            return EventResult::Ignored;
        }

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
            ScrollDirection::Left => self.scroll_x_by(-amount),
            ScrollDirection::Right => self.scroll_x_by(amount),
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
