//! Event handling for the Table widget.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::widgets::Block;

use crate::context::AppContext;
use crate::input::events::{Modifiers, ScrollDirection};
use crate::input::keybinds::{Key, KeyCombo};
use crate::runtime::render::layout::{apply_border, apply_padding};
use crate::widgets::events::{EventResult, WidgetEvent, WidgetEventKind, WidgetEvents};
use crate::widgets::scrollbar::{
    ScrollbarState, handle_scrollbar_click, handle_scrollbar_drag, handle_scrollbar_release,
    render_horizontal_scrollbar, render_vertical_scrollbar,
};
use crate::widgets::selection::SelectionMode;
use crate::widgets::traits::{AnyWidget, RenderContext, Scrollable, Selectable, SelectableWidget};

use super::any_table::AnyTable;
use super::item::TableRow;
use super::render::{render_header, render_row};
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
            cx.push_event(WidgetEvent::new(WidgetEventKind::Sort, self.id_string()));
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

impl<T: TableRow> WidgetEvents for Table<T> {
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

impl<T: TableRow + std::fmt::Debug> AnyWidget for Table<T> {
    fn id(&self) -> String {
        self.id_string()
    }

    fn is_dirty(&self) -> bool {
        Table::is_dirty(self)
    }

    fn clear_dirty(&self) {
        Table::clear_dirty(self)
    }

    fn is_focusable(&self) -> bool {
        true
    }

    fn captures_input(&self) -> bool {
        true // Table captures arrow keys, etc.
    }

    fn dispatch_click(&self, x: u16, y: u16, cx: &AppContext) -> EventResult {
        WidgetEvents::on_click(self, x, y, cx)
    }

    fn dispatch_key(&self, key: &KeyCombo, cx: &AppContext) -> EventResult {
        WidgetEvents::on_key(self, key, cx)
    }

    fn dispatch_hover(&self, x: u16, y: u16, cx: &AppContext) -> EventResult {
        WidgetEvents::on_hover(self, x, y, cx)
    }

    fn dispatch_scroll(
        &self,
        direction: crate::input::events::ScrollDirection,
        amount: u16,
        cx: &AppContext,
    ) -> EventResult {
        WidgetEvents::on_scroll(self, direction, amount, cx)
    }

    fn dispatch_drag(&self, x: u16, y: u16, modifiers: Modifiers, cx: &AppContext) -> EventResult {
        WidgetEvents::on_drag(self, x, y, modifiers, cx)
    }

    fn dispatch_release(&self, cx: &AppContext) -> EventResult {
        WidgetEvents::on_release(self, cx)
    }

    fn intrinsic_height(&self) -> u16 {
        // Table height = header (1 row) + number of data rows * row height
        let header_height = 1u16;
        let data_height = (self.len() as u16).saturating_mul(T::HEIGHT);
        header_height.saturating_add(data_height).max(1)
    }

    fn render(&self, frame: &mut Frame, area: Rect, _focused: bool, ctx: &mut RenderContext<'_>) {
        // Apply border and get inner area
        let (inner_area, block) = apply_border(area, &ctx.layout.border, ctx.style);
        if let Some(block) = block {
            frame.render_widget(block, area);
        } else if ctx.style.bg.is_some() {
            let bg_block = Block::default().style(ctx.style);
            frame.render_widget(bg_block, area);
        }

        // Apply padding
        let padded_area = apply_padding(inner_area, ctx.layout.padding);

        if padded_area.width == 0 || padded_area.height == 0 {
            return;
        }

        // Calculate effective table width (clamped to content)
        let table_content_width = self.total_width();

        // First pass: estimate viewport to determine scrollbar needs
        let estimated_width = padded_area.width.min(table_content_width);
        let estimated_height = padded_area.height;

        self.set_viewport_width(estimated_width);
        self.set_viewport_height(estimated_height);

        // Determine scrollbar needs based on content vs viewport
        let needs_v_scrollbar = self.needs_vertical_scrollbar();
        let needs_h_scrollbar = self.needs_horizontal_scrollbar();

        // Reserve space for scrollbars
        let v_scrollbar_reserved = if needs_v_scrollbar { 2u16 } else { 0u16 };
        let h_scrollbar_reserved = if needs_h_scrollbar { 1u16 } else { 0u16 };

        // Content area excludes scrollbars
        let content_area = Rect {
            x: padded_area.x,
            y: padded_area.y,
            width: padded_area.width.saturating_sub(v_scrollbar_reserved),
            height: padded_area.height.saturating_sub(h_scrollbar_reserved),
        };

        if content_area.width == 0 || content_area.height == 0 {
            return;
        }

        // Final viewport dimensions (clamped to content width)
        let effective_width = content_area.width.min(table_content_width);
        self.set_viewport_height(content_area.height);
        self.set_viewport_width(effective_width);

        // Use effective width for rendering (clamped to content)
        let render_area = Rect {
            x: content_area.x,
            y: content_area.y,
            width: effective_width,
            height: content_area.height,
        };

        // Header is at row 0, data starts at row 1
        let header_height = 1u16;
        let data_area = Rect {
            x: render_area.x,
            y: render_area.y + header_height,
            width: render_area.width,
            height: render_area.height.saturating_sub(header_height),
        };

        let columns = self.columns();
        let scroll_offset_x = self.scroll_offset_x();
        let scroll_offset_y = self.scroll_offset_y();
        let visible_col_range = self.visible_column_range();
        let visible_row_range = self.visible_row_range();
        let row_height = T::HEIGHT;

        // Render header row
        render_header(
            frame,
            &columns,
            self.sort(),
            Rect {
                x: render_area.x,
                y: render_area.y,
                width: render_area.width,
                height: header_height,
            },
            scroll_offset_x,
            visible_col_range.clone(),
            ctx.theme,
        );

        // Calculate column x-positions for rendering
        let mut col_positions: Vec<u16> = Vec::with_capacity(columns.len());
        let mut x_pos = 0u16;
        for col in &columns {
            col_positions.push(x_pos);
            x_pos += col.width;
        }

        // Render visible data rows
        let first_row_y =
            (visible_row_range.start as u16 * row_height).saturating_sub(scroll_offset_y);

        // Cast self to &dyn AnyTable for render_row
        let widget: &dyn AnyTable = self;

        for (i, row_index) in visible_row_range.clone().enumerate() {
            let row_y = data_area.y + first_row_y + (i as u16 * row_height);

            // Skip if outside viewport
            if row_y >= data_area.y + data_area.height {
                break;
            }

            let row_area = Rect {
                x: data_area.x,
                y: row_y,
                width: data_area.width,
                height: row_height.min(data_area.y + data_area.height - row_y),
            };

            // Render the row with cell-by-cell column widths
            render_row(
                frame,
                widget,
                row_index,
                &columns,
                &col_positions,
                scroll_offset_x,
                visible_col_range.clone(),
                row_area,
                ctx.theme,
            );
        }

        // Render vertical scrollbar if needed
        if needs_v_scrollbar {
            let scrollbar_area = Rect {
                x: render_area.x + render_area.width,
                y: render_area.y,
                width: 1,
                height: render_area.height.saturating_sub(h_scrollbar_reserved),
            };

            let config = ScrollbarState::scrollbar_config(self);
            let v_geom = render_vertical_scrollbar(
                frame.buffer_mut(),
                scrollbar_area,
                scroll_offset_y,
                self.total_height(),
                self.data_viewport_height(),
                &config,
                ctx.theme,
            );
            ScrollbarState::set_vertical_scrollbar(self, v_geom);
        } else {
            ScrollbarState::set_vertical_scrollbar(self, None);
        }

        // Render horizontal scrollbar if needed
        if needs_h_scrollbar {
            let scrollbar_area = Rect {
                x: render_area.x,
                y: render_area.y + render_area.height,
                width: render_area.width + v_scrollbar_reserved,
                height: 1,
            };

            let config = ScrollbarState::scrollbar_config(self);
            let h_geom = render_horizontal_scrollbar(
                frame.buffer_mut(),
                scrollbar_area,
                scroll_offset_x,
                self.total_width(),
                content_area.width,
                &config,
                ctx.theme,
            );
            AnyTable::set_horizontal_scrollbar(self, h_geom);
        } else {
            AnyTable::set_horizontal_scrollbar(self, None);
        }

        // Register hit box
        ctx.hit_map.register(self.id_string(), padded_area, true);
    }

    fn as_selectable(&self) -> Option<&dyn Selectable> {
        Some(self)
    }
}

// =============================================================================
// Scrollable trait implementation (capability trait for runtime)
// =============================================================================

impl<T: TableRow + std::fmt::Debug> Scrollable for Table<T> {
    fn scroll_offset(&self) -> usize {
        self.scroll_offset_y() as usize
    }

    fn set_scroll_offset(&self, offset: usize) {
        self.set_scroll_offset_y(offset as u16);
    }

    fn viewport_size(&self) -> usize {
        self.data_viewport_height() as usize
    }

    fn content_size(&self) -> usize {
        self.total_height() as usize
    }
}

// =============================================================================
// Selectable trait implementation (capability trait for runtime)
// =============================================================================

impl<T: TableRow + std::fmt::Debug> Selectable for Table<T> {
    fn cursor(&self) -> Option<usize> {
        SelectableWidget::cursor(self)
    }

    fn set_cursor(&self, index: usize) -> Option<usize> {
        SelectableWidget::set_cursor(self, index)
    }

    fn cursor_id(&self) -> Option<String> {
        SelectableWidget::cursor_id(self)
    }

    fn cursor_up(&self) -> Option<(Option<usize>, usize)> {
        SelectableWidget::cursor_up(self)
    }

    fn cursor_down(&self) -> Option<(Option<usize>, usize)> {
        SelectableWidget::cursor_down(self)
    }

    fn cursor_first(&self) -> Option<(Option<usize>, usize)> {
        SelectableWidget::cursor_first(self)
    }

    fn cursor_last(&self) -> Option<(Option<usize>, usize)> {
        SelectableWidget::cursor_last(self)
    }

    fn scroll_to_cursor(&self) {
        SelectableWidget::scroll_to_cursor(self)
    }

    fn selection_mode(&self) -> SelectionMode {
        SelectableWidget::selection_mode(self)
    }

    fn selected_ids(&self) -> Vec<String> {
        SelectableWidget::selected_ids(self)
    }

    fn toggle_select_at_cursor(&self) -> (Vec<String>, Vec<String>) {
        SelectableWidget::toggle_select_at_cursor(self)
    }

    fn select_all(&self) -> Vec<String> {
        SelectableWidget::select_all(self)
    }

    fn deselect_all(&self) -> Vec<String> {
        SelectableWidget::deselect_all(self)
    }

    fn item_count(&self) -> usize {
        SelectableWidget::item_count(self)
    }

    fn viewport_item_count(&self) -> usize {
        SelectableWidget::viewport_item_count(self)
    }

    fn item_height(&self) -> u16 {
        SelectableWidget::item_height(self)
    }

    fn has_header(&self) -> bool {
        true
    }

    fn on_header_click(&self, x_in_viewport: u16, cx: &AppContext) -> EventResult {
        Table::on_header_click(self, x_in_viewport, cx)
    }

    fn on_click_with_modifiers(
        &self,
        y_in_viewport: u16,
        ctrl: bool,
        shift: bool,
        cx: &AppContext,
    ) -> EventResult {
        // Table has a header row, so y=0 is header, y=1+ is data
        // The caller (handlers.rs) handles header clicks separately
        Table::on_row_click(self, y_in_viewport, ctrl, shift, cx)
    }
}
