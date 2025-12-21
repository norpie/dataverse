//! Event handling for the List widget.

use ratatui::layout::Rect;
use ratatui::widgets::Block;
use ratatui::Frame;

use crate::context::AppContext;
use crate::events::{Modifiers, ScrollDirection};
use crate::keybinds::KeyCombo;
use crate::runtime::render::layout::{apply_border, apply_padding};
use crate::widgets::events::{EventResult, WidgetEvents};
use crate::widgets::scrollbar::{
    handle_scrollbar_click, handle_scrollbar_drag, handle_scrollbar_release,
    render_vertical_scrollbar, ScrollbarState,
};
use crate::widgets::traits::{AnyWidget, RenderContext, SelectableWidget};

use super::item::ListItem;
use super::state::List;
use super::SelectionMode;

impl<T: ListItem> WidgetEvents for List<T> {
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

impl<T: ListItem + std::fmt::Debug> AnyWidget for List<T> {
    fn id(&self) -> String {
        self.id_string()
    }

    fn is_dirty(&self) -> bool {
        List::is_dirty(self)
    }

    fn clear_dirty(&self) {
        List::clear_dirty(self)
    }

    fn is_focusable(&self) -> bool {
        true
    }

    fn captures_input(&self) -> bool {
        true // List captures arrow keys, etc.
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
        // List height = number of items * item height
        (self.len() as u16).saturating_mul(T::HEIGHT).max(1)
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

            // Render the item
            if let Some(item) = self.get(index) {
                let is_cursor = self.cursor() == Some(index);
                let is_selected = self.is_selected(index);
                let item_node = item.render(is_cursor, is_selected);
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
}
