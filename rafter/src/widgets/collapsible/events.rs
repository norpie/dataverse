//! Event handling for the Collapsible widget.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::context::AppContext;
use crate::input::keybinds::{Key, KeyCombo};
use crate::widgets::events::{EventResult, WidgetEvent, WidgetEventKind, WidgetEvents};
use crate::widgets::traits::{AnyWidget, RenderContext};

use super::Collapsible;

/// Header height in rows (always 1)
const HEADER_HEIGHT: u16 = 1;

impl WidgetEvents for Collapsible {
    fn on_key(&self, key: &KeyCombo, cx: &AppContext) -> EventResult {
        // Only handle keys without modifiers
        if key.modifiers.ctrl || key.modifiers.alt || key.modifiers.shift {
            return EventResult::Ignored;
        }

        match key.key {
            Key::Char(' ') | Key::Enter => {
                self.toggle();
                let event_kind = if self.is_expanded() {
                    WidgetEventKind::Expand
                } else {
                    WidgetEventKind::Collapse
                };
                cx.push_event(WidgetEvent::new(event_kind, self.id_string()));
                EventResult::Consumed
            }
            _ => EventResult::Ignored,
        }
    }
}

impl AnyWidget for Collapsible {
    fn id(&self) -> String {
        self.id_string()
    }

    fn is_dirty(&self) -> bool {
        Collapsible::is_dirty(self)
    }

    fn clear_dirty(&self) {
        Collapsible::clear_dirty(self)
    }

    fn is_focusable(&self) -> bool {
        true
    }

    fn captures_input(&self) -> bool {
        false
    }

    fn intrinsic_height(&self) -> u16 {
        HEADER_HEIGHT
    }

    fn stacks_children(&self) -> bool {
        // Collapsible stacks header + children vertically
        true
    }

    fn registers_own_hit_area(&self) -> bool {
        // Collapsible only registers the header as clickable, not the full area
        // This allows children to receive their own click events
        true
    }

    fn intrinsic_width(&self) -> u16 {
        // Width is indicator + space + title
        let title = self.title();
        if title.is_empty() {
            2 // Just the indicator and space
        } else {
            (title.len() + 2) as u16
        }
    }

    fn dispatch_click(&self, _x: u16, y: u16, cx: &AppContext) -> EventResult {
        // Only toggle on header click (y == 0)
        if y == 0 {
            self.toggle();
            let event_kind = if self.is_expanded() {
                WidgetEventKind::Expand
            } else {
                WidgetEventKind::Collapse
            };
            cx.push_event(WidgetEvent::new(event_kind, self.id_string()));
            EventResult::Consumed
        } else {
            // Click on content area - pass through to children
            EventResult::Ignored
        }
    }

    fn dispatch_key(&self, key: &KeyCombo, cx: &AppContext) -> EventResult {
        self.on_key(key, cx)
    }

    fn render(&self, frame: &mut Frame, area: Rect, focused: bool, ctx: &mut RenderContext<'_>) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        let is_expanded = self.is_expanded();
        let indicator = if is_expanded {
            self.expanded_char()
        } else {
            self.collapsed_char()
        };
        let title = self.title();

        // Build header line
        let header_text = format!("{} {}", indicator, title);
        // Focused state gets a subtle background highlight (same as checkbox)
        let header_style = if focused {
            ctx.style
                .bg(Color::Rgb(80, 80, 100))
                .add_modifier(ratatui::style::Modifier::BOLD)
        } else {
            ctx.style
        };

        // Render header
        let header_area = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: HEADER_HEIGHT.min(area.height),
        };

        let header_line = Line::from(vec![Span::styled(header_text, header_style)]);
        let header_paragraph = Paragraph::new(header_line);
        frame.render_widget(header_paragraph, header_area);

        // Register header as clickable hit area
        ctx.hit_map.register(self.id_string(), header_area, false);

        // Render children only when expanded and there's space
        if is_expanded && area.height > HEADER_HEIGHT && !ctx.children.is_empty() {
            let content_area = Rect {
                x: area.x,
                y: area.y + HEADER_HEIGHT,
                width: area.width,
                height: area.height - HEADER_HEIGHT,
            };

            // Render each child in a column layout
            let mut current_y = content_area.y;
            for child in ctx.children {
                if current_y >= content_area.y + content_area.height {
                    break;
                }

                let child_height = child.intrinsic_height().max(1);
                let remaining_height =
                    (content_area.y + content_area.height).saturating_sub(current_y);
                let actual_height = child_height.min(remaining_height);

                if actual_height == 0 {
                    break;
                }

                let child_area = Rect {
                    x: content_area.x,
                    y: current_y,
                    width: content_area.width,
                    height: actual_height,
                };

                (ctx.render_node)(
                    frame,
                    child,
                    child_area,
                    ctx.hit_map,
                    ctx.theme,
                    ctx.focused_id,
                    ctx.overlay_requests,
                );

                current_y += actual_height;
            }
        }
    }
}
