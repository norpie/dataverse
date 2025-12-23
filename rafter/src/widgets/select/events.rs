//! Event handling for the Select widget.

use ratatui::Frame;
use ratatui::layout::Rect;

use crate::context::AppContext;
use crate::input::events::ScrollDirection;
use crate::input::keybinds::{Key, KeyCombo};
use crate::layers::overlay::{OverlayPosition, OverlayRequest};
use crate::node::Node;
use crate::widgets::events::{EventResult, WidgetEvent, WidgetEventKind, WidgetEvents};
use crate::widgets::traits::{AnyWidget, RenderContext};

use super::Select;
use super::render;

impl WidgetEvents for Select {
    fn on_key(&self, key: &KeyCombo, cx: &AppContext) -> EventResult {
        // Ignore keys with ctrl/alt modifiers
        if key.modifiers.ctrl || key.modifiers.alt {
            return EventResult::Ignored;
        }

        if !self.is_open() {
            // Closed state - open on Enter, Space, or Down
            match key.key {
                Key::Enter | Key::Char(' ') | Key::Down => {
                    self.open();
                    EventResult::Consumed
                }
                _ => EventResult::Ignored,
            }
        } else {
            // Open state - navigate and select
            match key.key {
                Key::Up => {
                    self.cursor_up();
                    EventResult::Consumed
                }
                Key::Down => {
                    self.cursor_down();
                    EventResult::Consumed
                }
                Key::Enter | Key::Char(' ') => {
                    // Select current cursor position
                    let cursor = self.cursor();
                    self.set_selected_index(Some(cursor));
                    self.close();
                    cx.push_event(WidgetEvent::new(WidgetEventKind::Change, self.id_string()));
                    EventResult::Consumed
                }
                Key::Escape => {
                    self.close();
                    EventResult::Consumed
                }
                Key::Home => {
                    self.set_cursor(0);
                    EventResult::Consumed
                }
                Key::End => {
                    let max = self.options_count().saturating_sub(1);
                    self.set_cursor(max);
                    EventResult::Consumed
                }
                _ => EventResult::Ignored,
            }
        }
    }
}

impl AnyWidget for Select {
    fn id(&self) -> String {
        self.id_string()
    }

    fn is_dirty(&self) -> bool {
        Select::is_dirty(self)
    }

    fn clear_dirty(&self) {
        Select::clear_dirty(self)
    }

    fn is_focusable(&self) -> bool {
        true
    }

    fn captures_input(&self) -> bool {
        true // Select captures keyboard input when focused
    }

    fn intrinsic_height(&self) -> u16 {
        1 // Trigger is one line tall
    }

    fn hides_children_from_layout(&self) -> bool {
        true // Children are for overlay content, not layout
    }

    fn intrinsic_width(&self) -> u16 {
        // Width based on selected label or placeholder, plus dropdown indicator
        let selected = self.selected_label();
        let placeholder = self.placeholder();
        let content = selected.as_deref().unwrap_or(&placeholder);
        (content.len() + 2).max(10) as u16 // +2 for dropdown arrow and space
    }

    fn dispatch_click(&self, _x: u16, _y: u16, _cx: &AppContext) -> EventResult {
        // Click toggles the dropdown
        if self.is_open() {
            self.close();
        } else {
            self.open();
        }
        // No change event on toggle - only on actual selection
        EventResult::Consumed
    }

    fn dispatch_key(&self, key: &KeyCombo, cx: &AppContext) -> EventResult {
        self.on_key(key, cx)
    }

    fn dispatch_blur(&self, _cx: &AppContext) {
        // Close dropdown when focus leaves
        self.close();
    }

    fn dispatch_overlay_click(&self, _x: u16, y: u16, cx: &AppContext) -> EventResult {
        if !self.is_open() {
            return EventResult::Ignored;
        }

        // y coordinate maps directly to option index (no padding)
        let index = y as usize;

        if index < self.options_count() {
            // Select the clicked option
            self.set_selected_index(Some(index));
            self.close();
            cx.push_event(WidgetEvent::new(WidgetEventKind::Change, self.id_string()));
            EventResult::Consumed
        } else {
            // Click was outside options
            EventResult::Consumed
        }
    }

    fn dispatch_overlay_scroll(
        &self,
        direction: ScrollDirection,
        _amount: u16,
        _cx: &AppContext,
    ) -> EventResult {
        if !self.is_open() {
            return EventResult::Ignored;
        }

        // Scroll moves the cursor in the dropdown
        match direction {
            ScrollDirection::Up => {
                self.cursor_up();
                EventResult::Consumed
            }
            ScrollDirection::Down => {
                self.cursor_down();
                EventResult::Consumed
            }
            _ => EventResult::Ignored,
        }
    }

    fn dispatch_overlay_hover(&self, _x: u16, y: u16, _cx: &AppContext) -> EventResult {
        if !self.is_open() {
            return EventResult::Ignored;
        }

        // y coordinate maps directly to option index
        let index = y as usize;

        if index < self.options_count() {
            // Move cursor to hovered option
            self.set_cursor(index);
            EventResult::Consumed
        } else {
            // Hover is on bottom spacer, ignore
            EventResult::Ignored
        }
    }

    fn render(&self, frame: &mut Frame, area: Rect, focused: bool, ctx: &mut RenderContext<'_>) {
        // Store anchor for overlay positioning
        self.set_anchor_rect(area);

        // Extract option labels from children for display
        let labels: Vec<String> = ctx
            .children
            .iter()
            .filter_map(|child| {
                if let Node::Text { content, .. } = child {
                    Some(content.clone())
                } else {
                    None
                }
            })
            .collect();

        self.set_options_count(labels.len());
        self.set_option_labels(labels);

        // Render the trigger (closed select appearance)
        render::render_trigger(frame, area, self, focused, ctx);

        // If open, register overlay for the dropdown
        if self.is_open() {
            let dropdown_content = render::build_dropdown_content(self, ctx);
            ctx.register_overlay(OverlayRequest {
                owner_id: self.id_string(),
                content: dropdown_content,
                anchor: area,
                position: OverlayPosition::Below,
            });
        }
    }
}
