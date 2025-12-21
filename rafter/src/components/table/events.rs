//! Event handling for the Table component.

use crate::components::events::{ComponentEvent, ComponentEventKind, ComponentEvents, EventResult};
use crate::components::scrollbar::{
    ScrollbarState, handle_scrollbar_click, handle_scrollbar_drag, handle_scrollbar_release,
};
use crate::components::selection::SelectionMode;
use crate::components::traits::SelectableComponent;
use crate::context::AppContext;
use crate::events::{Modifiers, ScrollDirection};
use crate::keybinds::{Key, KeyCombo};

use super::item::TableRow;
use super::state::Table;

/// Horizontal scroll amount per key press (in terminal columns).
const HORIZONTAL_SCROLL_AMOUNT: i16 = 10;

impl<T: TableRow> Table<T> {
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
            cx.set_sorted(col, asc);
            cx.push_event(ComponentEvent::new(
                ComponentEventKind::Sort,
                self.id_string(),
            ));
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
                self.push_activate_event(cx);
            }
            SelectionMode::Single => {
                if ctrl {
                    let (added, removed) = self.toggle_select(&id);
                    self.push_selection_event(&added, &removed, cx);
                } else {
                    self.push_activate_event(cx);
                }
            }
            SelectionMode::Multiple => {
                if shift {
                    let (added, removed) = self.range_select(&id, ctrl);
                    self.push_selection_event(&added, &removed, cx);
                } else if ctrl {
                    let (added, removed) = self.toggle_select(&id);
                    self.push_selection_event(&added, &removed, cx);
                } else {
                    self.push_activate_event(cx);
                }
            }
        }

        self.scroll_to_cursor();
        EventResult::Consumed
    }
}

impl<T: TableRow> ComponentEvents for Table<T> {
    fn on_key(&self, key: &KeyCombo, cx: &AppContext) -> EventResult {
        // Table-specific: Horizontal scrolling with Left/Right
        match key.key {
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
            _ => {}
        }

        // Try shared navigation keys (Up, Down, Home, End, PageUp, PageDown)
        if let Some(result) = self.handle_navigation_key(key, cx) {
            return result;
        }

        // Try shared selection keys (Space, Ctrl+A, Escape, Enter)
        if let Some(result) = self.handle_selection_key(key, cx) {
            return result;
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

        // Use the trait's handle_hover which already handles the header offset
        // via our overridden index_from_viewport_y
        self.handle_hover(y, cx)
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
