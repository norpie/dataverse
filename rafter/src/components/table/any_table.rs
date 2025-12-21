//! Type-erased table trait for use in Node.

use std::any::Any;
use std::ops::Range;

use crate::components::events::{ComponentEvents, EventResult};
use crate::components::scrollbar::{
    ScrollbarConfig, ScrollbarDrag, ScrollbarGeometry, ScrollbarState,
};
use crate::context::AppContext;
use crate::events::{Modifiers, ScrollDirection};
use crate::keybinds::KeyCombo;
use crate::node::Node;

use super::item::{Column, TableRow};
use super::state::Table;

/// Type-erased table operations for use in Node.
///
/// This trait allows the `Node::Table` variant to work with any `Table<T>`
/// without knowing the concrete row type.
pub trait AnyTable: Send + Sync + std::fmt::Debug {
    /// Get the table ID as a string.
    fn id_string(&self) -> String;

    /// Get the row height.
    fn row_height(&self) -> u16;

    /// Get the number of rows.
    fn len(&self) -> usize;

    /// Check if empty.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get the cursor position (row index).
    fn cursor(&self) -> Option<usize>;

    /// Get the cursor row ID.
    fn cursor_id(&self) -> Option<String>;

    /// Check if a row at index is selected.
    fn is_selected_at(&self, row_index: usize) -> bool;

    // -------------------------------------------------------------------------
    // Column access
    // -------------------------------------------------------------------------

    /// Get column definitions.
    fn columns(&self) -> Vec<Column>;

    /// Get the number of columns.
    fn column_count(&self) -> usize;

    /// Get total content width (sum of all column widths).
    fn total_width(&self) -> u16;

    /// Get current sort state (column index, ascending).
    fn sort(&self) -> Option<(usize, bool)>;

    // -------------------------------------------------------------------------
    // Vertical scrolling
    // -------------------------------------------------------------------------

    /// Get the vertical scroll offset (in rows).
    fn scroll_offset_y(&self) -> u16;

    /// Set the viewport height.
    fn set_viewport_height(&self, height: u16);

    /// Get the viewport height.
    fn viewport_height(&self) -> u16;

    /// Get the data viewport height (excluding header row).
    fn data_viewport_height(&self) -> u16;

    /// Get the visible row range.
    fn visible_row_range(&self) -> Range<usize>;

    /// Get total content height (rows only, not header).
    fn total_height(&self) -> u16;

    /// Check if vertical scrollbar is needed.
    fn needs_vertical_scrollbar(&self) -> bool;

    // -------------------------------------------------------------------------
    // Horizontal scrolling
    // -------------------------------------------------------------------------

    /// Get horizontal scroll offset in terminal columns.
    fn scroll_offset_x(&self) -> u16;

    /// Set the viewport width.
    fn set_viewport_width(&self, width: u16);

    /// Get the viewport width.
    fn viewport_width(&self) -> u16;

    /// Get the visible column range.
    fn visible_column_range(&self) -> Range<usize>;

    /// Check if horizontal scrollbar is needed.
    fn needs_horizontal_scrollbar(&self) -> bool;

    // -------------------------------------------------------------------------
    // Rendering
    // -------------------------------------------------------------------------

    /// Render a specific row (used for custom row styling).
    fn render_row(&self, row_index: usize) -> Option<Node>;

    /// Render a specific cell in a row.
    /// Returns the cell content Node without any container.
    fn render_cell(&self, row_index: usize, column_index: usize) -> Option<Node>;

    /// Check if a row is focused (has cursor).
    fn is_focused_at(&self, row_index: usize) -> bool;

    /// Clone as boxed trait object.
    fn clone_box(&self) -> Box<dyn AnyTable>;

    /// As Any for downcasting.
    fn as_any(&self) -> &dyn Any;

    // -------------------------------------------------------------------------
    // Event handlers
    // -------------------------------------------------------------------------

    /// Handle a key event.
    fn on_key(&self, key: &KeyCombo, cx: &AppContext) -> EventResult;

    /// Handle a click event at the given position within the table bounds.
    fn on_click(&self, x: u16, y: u16, cx: &AppContext) -> EventResult;

    /// Handle a hover event at the given position within the table bounds.
    fn on_hover(&self, x: u16, y: u16, cx: &AppContext) -> EventResult;

    /// Handle a scroll event.
    fn on_scroll(&self, direction: ScrollDirection, amount: u16, cx: &AppContext) -> EventResult;

    /// Handle a drag event.
    fn on_drag(&self, x: u16, y: u16, modifiers: Modifiers, cx: &AppContext) -> EventResult;

    /// Handle a release event.
    fn on_release(&self, cx: &AppContext) -> EventResult;

    /// Handle a click on a data row with modifier keys.
    fn on_row_click(
        &self,
        y_in_viewport: u16,
        ctrl: bool,
        shift: bool,
        cx: &AppContext,
    ) -> EventResult;

    /// Handle a click on a header cell (for sorting).
    fn on_header_click(&self, x: u16, cx: &AppContext) -> EventResult;

    // -------------------------------------------------------------------------
    // Scrollbar support
    // -------------------------------------------------------------------------

    /// Get the scrollbar configuration.
    fn scrollbar_config(&self) -> ScrollbarConfig;

    /// Get the vertical scrollbar geometry.
    fn vertical_scrollbar(&self) -> Option<ScrollbarGeometry>;

    /// Set the vertical scrollbar geometry.
    fn set_vertical_scrollbar(&self, geometry: Option<ScrollbarGeometry>);

    /// Get the horizontal scrollbar geometry.
    fn horizontal_scrollbar(&self) -> Option<ScrollbarGeometry>;

    /// Set the horizontal scrollbar geometry.
    fn set_horizontal_scrollbar(&self, geometry: Option<ScrollbarGeometry>);

    /// Scroll to a vertical position based on a ratio (0.0 - 1.0).
    fn scroll_to_ratio_y(&self, ratio: f32);

    /// Scroll to a horizontal position based on a ratio (0.0 - 1.0).
    fn scroll_to_ratio_x(&self, ratio: f32);

    /// Get current drag state.
    fn drag(&self) -> Option<ScrollbarDrag>;

    /// Set current drag state.
    fn set_drag(&self, drag: Option<ScrollbarDrag>);
}

impl<T: TableRow + std::fmt::Debug> AnyTable for Table<T> {
    fn id_string(&self) -> String {
        self.id_string()
    }

    fn row_height(&self) -> u16 {
        T::HEIGHT
    }

    fn len(&self) -> usize {
        Table::len(self)
    }

    fn cursor(&self) -> Option<usize> {
        Table::cursor(self)
    }

    fn cursor_id(&self) -> Option<String> {
        Table::cursor_id(self)
    }

    fn is_selected_at(&self, row_index: usize) -> bool {
        Table::is_selected_at(self, row_index)
    }

    fn columns(&self) -> Vec<Column> {
        Table::columns(self)
    }

    fn column_count(&self) -> usize {
        Table::column_count(self)
    }

    fn total_width(&self) -> u16 {
        Table::total_width(self)
    }

    fn sort(&self) -> Option<(usize, bool)> {
        Table::sort(self)
    }

    fn scroll_offset_y(&self) -> u16 {
        Table::scroll_offset_y(self)
    }

    fn set_viewport_height(&self, height: u16) {
        Table::set_viewport_height(self, height);
    }

    fn viewport_height(&self) -> u16 {
        Table::viewport_height(self)
    }

    fn data_viewport_height(&self) -> u16 {
        Table::data_viewport_height(self)
    }

    fn visible_row_range(&self) -> Range<usize> {
        Table::visible_row_range(self)
    }

    fn total_height(&self) -> u16 {
        Table::total_height(self)
    }

    fn needs_vertical_scrollbar(&self) -> bool {
        Table::needs_vertical_scrollbar(self)
    }

    fn scroll_offset_x(&self) -> u16 {
        Table::scroll_offset_x(self)
    }

    fn set_viewport_width(&self, width: u16) {
        Table::set_viewport_width(self, width);
    }

    fn viewport_width(&self) -> u16 {
        Table::viewport_width(self)
    }

    fn visible_column_range(&self) -> Range<usize> {
        Table::visible_column_range(self)
    }

    fn needs_horizontal_scrollbar(&self) -> bool {
        Table::needs_horizontal_scrollbar(self)
    }

    fn render_row(&self, row_index: usize) -> Option<Node> {
        let row = self.row(row_index)?;
        let is_focused = self.cursor() == Some(row_index);
        let is_selected = self.is_selected_at(row_index);

        // Render cells for all columns
        let cells: Vec<Node> = (0..row.column_count())
            .filter_map(|col_idx| row.render_cell(col_idx, is_focused, is_selected))
            .collect();

        Some(row.render_row(cells, is_focused, is_selected))
    }

    fn render_cell(&self, row_index: usize, column_index: usize) -> Option<Node> {
        let row = self.row(row_index)?;
        let is_focused = self.cursor() == Some(row_index);
        let is_selected = self.is_selected_at(row_index);
        row.render_cell(column_index, is_focused, is_selected)
    }

    fn is_focused_at(&self, row_index: usize) -> bool {
        self.cursor() == Some(row_index)
    }

    fn clone_box(&self) -> Box<dyn AnyTable> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn on_key(&self, key: &KeyCombo, cx: &AppContext) -> EventResult {
        ComponentEvents::on_key(self, key, cx)
    }

    fn on_click(&self, x: u16, y: u16, cx: &AppContext) -> EventResult {
        ComponentEvents::on_click(self, x, y, cx)
    }

    fn on_hover(&self, x: u16, y: u16, cx: &AppContext) -> EventResult {
        ComponentEvents::on_hover(self, x, y, cx)
    }

    fn on_scroll(&self, direction: ScrollDirection, amount: u16, cx: &AppContext) -> EventResult {
        ComponentEvents::on_scroll(self, direction, amount, cx)
    }

    fn on_drag(&self, x: u16, y: u16, modifiers: Modifiers, cx: &AppContext) -> EventResult {
        ComponentEvents::on_drag(self, x, y, modifiers, cx)
    }

    fn on_release(&self, cx: &AppContext) -> EventResult {
        ComponentEvents::on_release(self, cx)
    }

    fn on_row_click(
        &self,
        y_in_viewport: u16,
        ctrl: bool,
        shift: bool,
        cx: &AppContext,
    ) -> EventResult {
        Table::on_row_click(self, y_in_viewport, ctrl, shift, cx)
    }

    fn on_header_click(&self, x: u16, cx: &AppContext) -> EventResult {
        Table::on_header_click(self, x, cx)
    }

    fn scrollbar_config(&self) -> ScrollbarConfig {
        ScrollbarState::scrollbar_config(self)
    }

    fn vertical_scrollbar(&self) -> Option<ScrollbarGeometry> {
        ScrollbarState::vertical_scrollbar(self)
    }

    fn set_vertical_scrollbar(&self, geometry: Option<ScrollbarGeometry>) {
        ScrollbarState::set_vertical_scrollbar(self, geometry);
    }

    fn horizontal_scrollbar(&self) -> Option<ScrollbarGeometry> {
        self.inner
            .read()
            .ok()
            .and_then(|guard| guard.horizontal_scrollbar)
    }

    fn set_horizontal_scrollbar(&self, geometry: Option<ScrollbarGeometry>) {
        if let Ok(mut guard) = self.inner.write() {
            guard.horizontal_scrollbar = geometry;
        }
    }

    fn scroll_to_ratio_y(&self, ratio: f32) {
        ScrollbarState::scroll_to_ratio(self, None, Some(ratio));
    }

    fn scroll_to_ratio_x(&self, ratio: f32) {
        let max_offset = self.total_width().saturating_sub(self.viewport_width());
        let offset = (max_offset as f32 * ratio.clamp(0.0, 1.0)) as u16;
        self.set_scroll_offset_x(offset);
    }

    fn drag(&self) -> Option<ScrollbarDrag> {
        ScrollbarState::drag(self)
    }

    fn set_drag(&self, drag: Option<ScrollbarDrag>) {
        ScrollbarState::set_drag(self, drag);
    }
}

impl Clone for Box<dyn AnyTable> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}
