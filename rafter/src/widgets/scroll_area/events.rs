//! Event handling for the ScrollArea widget.

use ratatui::layout::Rect;
use ratatui::widgets::Block;
use ratatui::Frame;

use crate::context::AppContext;
use crate::events::{Modifiers, ScrollDirection};
use crate::keybinds::{Key, KeyCombo};
use crate::widgets::events::{EventResult, WidgetEvents};
use crate::widgets::scrollbar::{
    handle_scroll, handle_scrollbar_click, handle_scrollbar_drag, handle_scrollbar_release,
    render_horizontal_scrollbar, render_vertical_scrollbar, ScrollbarState,
};
use crate::widgets::traits::{AnyWidget, RenderContext};

use super::render::{
    calculate_scroll_area_layout, calculate_wrapped_content_size, render_node_clipped, ClipRect,
};
use super::ScrollArea;

/// Lines to scroll per arrow key press.
const SCROLL_LINES: i16 = 1;
/// Lines to scroll per page up/down.
const SCROLL_PAGE_LINES: i16 = 10;

impl WidgetEvents for ScrollArea {
    fn on_click(&self, x: u16, y: u16, cx: &AppContext) -> EventResult {
        // Delegate to shared scrollbar click handling
        handle_scrollbar_click(self, x, y, cx).unwrap_or(EventResult::Ignored)
    }

    fn on_scroll(&self, direction: ScrollDirection, amount: u16, cx: &AppContext) -> EventResult {
        handle_scroll(self, direction, amount, cx)
    }

    fn on_drag(&self, x: u16, y: u16, modifiers: Modifiers, cx: &AppContext) -> EventResult {
        handle_scrollbar_drag(self, x, y, modifiers, cx)
    }

    fn on_release(&self, cx: &AppContext) -> EventResult {
        handle_scrollbar_release(self, cx)
    }

    fn on_key(&self, key: &KeyCombo, _cx: &AppContext) -> EventResult {
        // Ignore keys with ctrl/alt modifiers (let app handle those)
        if key.modifiers.ctrl || key.modifiers.alt {
            return EventResult::Ignored;
        }

        match key.key {
            Key::Up => {
                self.scroll_by(0, -SCROLL_LINES);
                EventResult::Consumed
            }
            Key::Down => {
                self.scroll_by(0, SCROLL_LINES);
                EventResult::Consumed
            }
            Key::Left => {
                self.scroll_by(-SCROLL_LINES, 0);
                EventResult::Consumed
            }
            Key::Right => {
                self.scroll_by(SCROLL_LINES, 0);
                EventResult::Consumed
            }
            Key::PageUp => {
                // Scroll by viewport height, or fallback to SCROLL_PAGE_LINES
                let viewport_height = ScrollbarState::viewport_height(self) as i16;
                let amount = if viewport_height > 0 {
                    viewport_height
                } else {
                    SCROLL_PAGE_LINES
                };
                self.scroll_by(0, -amount);
                EventResult::Consumed
            }
            Key::PageDown => {
                let viewport_height = ScrollbarState::viewport_height(self) as i16;
                let amount = if viewport_height > 0 {
                    viewport_height
                } else {
                    SCROLL_PAGE_LINES
                };
                self.scroll_by(0, amount);
                EventResult::Consumed
            }
            Key::Home => {
                self.scroll_to_top();
                EventResult::Consumed
            }
            Key::End => {
                self.scroll_to_bottom();
                EventResult::Consumed
            }
            _ => EventResult::Ignored,
        }
    }
}

impl AnyWidget for ScrollArea {
    fn id(&self) -> String {
        self.id_string()
    }

    fn is_dirty(&self) -> bool {
        ScrollArea::is_dirty(self)
    }

    fn clear_dirty(&self) {
        ScrollArea::clear_dirty(self)
    }

    fn is_focusable(&self) -> bool {
        true
    }

    fn captures_input(&self) -> bool {
        false
    }

    fn dispatch_click(&self, x: u16, y: u16, cx: &AppContext) -> EventResult {
        self.on_click(x, y, cx)
    }

    fn dispatch_key(&self, key: &KeyCombo, cx: &AppContext) -> EventResult {
        self.on_key(key, cx)
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

    fn render(&self, frame: &mut Frame, area: Rect, _focused: bool, ctx: &mut RenderContext<'_>) {
        // ScrollArea expects exactly one child
        let child = match ctx.children.first() {
            Some(c) => c,
            None => return, // Nothing to render without a child
        };

        // Fill background if specified
        if ctx.style.bg.is_some() {
            let block = Block::default().style(ctx.style);
            frame.render_widget(block, area);
        }

        // Calculate layout first to get viewport dimensions
        let initial_content_size = (child.intrinsic_width(), child.intrinsic_height());
        let scroll_layout = calculate_scroll_area_layout(
            area,
            initial_content_size,
            self.direction(),
            &self.scrollbar_config(),
        );

        // Calculate actual content height with wrapping based on viewport width
        let content_size = calculate_wrapped_content_size(child, scroll_layout.content_area.width);

        // Update widget with computed sizes
        self.set_sizes(
            content_size,
            (
                scroll_layout.content_area.width,
                scroll_layout.content_area.height,
            ),
        );

        // Get scroll offset
        let (offset_x, offset_y) = self.offset();

        // Render scrollbars and save geometry for hit testing
        let v_geom = if scroll_layout.show_vertical {
            render_vertical_scrollbar(
                frame.buffer_mut(),
                area,
                offset_y,
                content_size.1,
                scroll_layout.content_area.height,
                &self.scrollbar_config(),
                ctx.theme,
            )
        } else {
            None
        };
        self.set_vertical_scrollbar(v_geom);

        let h_geom = if scroll_layout.show_horizontal {
            render_horizontal_scrollbar(
                frame.buffer_mut(),
                area,
                offset_x,
                content_size.0,
                scroll_layout.content_area.width,
                &self.scrollbar_config(),
                ctx.theme,
            )
        } else {
            None
        };
        self.set_horizontal_scrollbar(h_geom);

        // Render child with viewport clipping
        let viewport = scroll_layout.content_area;

        if viewport.width > 0 && viewport.height > 0 {
            let clip = ClipRect {
                viewport,
                offset_x,
                offset_y,
            };

            render_node_clipped(
                frame,
                child,
                viewport,
                &clip,
                ctx.hit_map,
                ctx.theme,
                ctx.focused_id,
                crate::runtime::render::style_to_ratatui,
                ctx.render_node,
            );
        }

        // Register hit box for scroll area (focusable for keyboard navigation)
        ctx.hit_map.register(self.id_string(), area, true);
    }
}
