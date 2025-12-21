//! Event handling for the Tree widget.

use ratatui::layout::Rect;
use ratatui::widgets::Block;
use ratatui::Frame;

use crate::context::AppContext;
use crate::events::{Modifiers, ScrollDirection};
use crate::keybinds::{Key, KeyCombo};
use crate::runtime::render::layout::{apply_border, apply_padding};
use crate::widgets::events::{EventResult, WidgetEvent, WidgetEventKind, WidgetEvents};
use crate::widgets::scrollbar::{
    handle_scrollbar_click, handle_scrollbar_drag, handle_scrollbar_release,
    render_vertical_scrollbar, ScrollbarState,
};
use crate::widgets::selection::SelectionMode;
use crate::widgets::traits::{AnyWidget, RenderContext, Scrollable, Selectable, SelectableWidget};

use super::item::TreeItem;
use super::state::Tree;

impl<T: TreeItem> Tree<T> {
    /// Handle expand event, pushing event.
    fn handle_expand(&self, node_id: &str, cx: &AppContext) {
        cx.set_expanded(node_id.to_string());
        cx.push_event(WidgetEvent::new(
            WidgetEventKind::Expand,
            self.id_string(),
        ));
    }

    /// Handle collapse event, pushing event.
    fn handle_collapse(&self, node_id: &str, cx: &AppContext) {
        cx.set_collapsed(node_id.to_string());
        cx.push_event(WidgetEvent::new(
            WidgetEventKind::Collapse,
            self.id_string(),
        ));
    }
}

impl<T: TreeItem> WidgetEvents for Tree<T> {
    fn on_key(&self, key: &KeyCombo, cx: &AppContext) -> EventResult {
        // Tree-specific: Expand/Collapse with Left/Right arrows
        match key.key {
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
                        if self.cursor_to_parent().is_some() {
                            self.scroll_to_cursor();
                            self.push_cursor_event(cx);
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
                        if self.cursor_to_first_child().is_some() {
                            self.scroll_to_cursor();
                            self.push_cursor_event(cx);
                            return EventResult::Consumed;
                        }
                    }
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

impl<T: TreeItem + std::fmt::Debug> AnyWidget for Tree<T> {
    fn id(&self) -> String {
        self.id_string()
    }

    fn is_dirty(&self) -> bool {
        Tree::is_dirty(self)
    }

    fn clear_dirty(&self) {
        Tree::clear_dirty(self)
    }

    fn is_focusable(&self) -> bool {
        true
    }

    fn captures_input(&self) -> bool {
        true // Tree captures arrow keys, etc.
    }

    fn dispatch_click(&self, x: u16, y: u16, cx: &AppContext) -> EventResult {
        self.on_click(x, y, cx)
    }

    fn dispatch_key(&self, key: &KeyCombo, cx: &AppContext) -> EventResult {
        self.on_key(key, cx)
    }

    fn dispatch_hover(&self, x: u16, y: u16, cx: &AppContext) -> EventResult {
        self.on_hover(x, y, cx)
    }

    fn dispatch_scroll(
        &self,
        direction: crate::events::ScrollDirection,
        amount: u16,
        cx: &AppContext,
    ) -> EventResult {
        self.on_scroll(direction, amount, cx)
    }

    fn dispatch_drag(&self, x: u16, y: u16, modifiers: Modifiers, cx: &AppContext) -> EventResult {
        self.on_drag(x, y, modifiers, cx)
    }

    fn dispatch_release(&self, cx: &AppContext) -> EventResult {
        self.on_release(cx)
    }

    fn intrinsic_height(&self) -> u16 {
        // Tree height = number of visible nodes * item height
        self.total_height().max(1)
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

        // Determine if we need a scrollbar
        let needs_scrollbar = ScrollbarState::needs_vertical_scrollbar(self);
        let scrollbar_reserved = if needs_scrollbar { 2u16 } else { 0u16 };

        // Content area excludes scrollbar
        let content_area = Rect {
            x: padded_area.x,
            y: padded_area.y,
            width: padded_area.width.saturating_sub(scrollbar_reserved),
            height: padded_area.height,
        };

        if content_area.width == 0 || content_area.height == 0 {
            return;
        }

        // Update viewport height
        self.set_viewport_height(content_area.height);

        // Get visible range
        let visible_range = self.visible_range();
        let item_height = T::HEIGHT;
        let scroll_offset = self.scroll_offset();

        // Calculate offset for first visible item
        let first_item_y = (visible_range.start as u16 * item_height).saturating_sub(scroll_offset);

        // Render visible items
        for (i, index) in visible_range.enumerate() {
            let item_y = content_area.y + first_item_y + (i as u16 * item_height);

            if item_y >= content_area.y + content_area.height {
                break;
            }

            let item_area = Rect {
                x: content_area.x,
                y: item_y,
                width: content_area.width,
                height: item_height.min(content_area.y + content_area.height - item_y),
            };

            // Render the node
            if let Some(node) = self.visible_node(index) {
                let is_cursor = self.cursor() == Some(index);
                let is_selected = self.is_selected_at(index);
                let item_node =
                    node.item
                        .render(is_cursor, is_selected, node.depth, node.is_expanded);
                (ctx.render_node)(
                    frame,
                    &item_node,
                    item_area,
                    ctx.hit_map,
                    ctx.theme,
                    ctx.focused_id,
                );
            }
        }

        // Render vertical scrollbar if needed
        if needs_scrollbar {
            let scrollbar_area = Rect {
                x: padded_area.x + padded_area.width - 1,
                y: padded_area.y,
                width: 1,
                height: padded_area.height,
            };

            let config = ScrollbarState::scrollbar_config(self);
            let v_geom = render_vertical_scrollbar(
                frame.buffer_mut(),
                scrollbar_area,
                scroll_offset,
                self.total_height(),
                content_area.height,
                &config,
                ctx.theme,
            );
            ScrollbarState::set_vertical_scrollbar(self, v_geom);
        } else {
            ScrollbarState::set_vertical_scrollbar(self, None);
        }

        // Register hit box
        ctx.hit_map
            .register(self.id_string(), padded_area, true);
    }

    fn as_selectable(&self) -> Option<&dyn Selectable> {
        Some(self)
    }
}

// =============================================================================
// Scrollable trait implementation (capability trait for runtime)
// =============================================================================

impl<T: TreeItem + std::fmt::Debug> Scrollable for Tree<T> {
    fn scroll_offset(&self) -> usize {
        Tree::scroll_offset(self) as usize
    }

    fn set_scroll_offset(&self, offset: usize) {
        Tree::set_scroll_offset(self, offset as u16);
    }

    fn viewport_size(&self) -> usize {
        Tree::viewport_height(self) as usize
    }

    fn content_size(&self) -> usize {
        self.total_height() as usize
    }
}

// =============================================================================
// Selectable trait implementation (capability trait for runtime)
// =============================================================================

impl<T: TreeItem + std::fmt::Debug> Selectable for Tree<T> {
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

    fn on_click_with_modifiers(
        &self,
        y_in_viewport: u16,
        ctrl: bool,
        shift: bool,
        cx: &AppContext,
    ) -> EventResult {
        Tree::on_click_with_modifiers(self, y_in_viewport, ctrl, shift, cx)
    }
}
