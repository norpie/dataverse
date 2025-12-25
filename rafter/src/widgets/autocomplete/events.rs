//! Event handling for the Autocomplete widget.

use ratatui::Frame;
use ratatui::layout::Rect;

use crate::context::AppContext;
use crate::input::events::ScrollDirection;
use crate::input::keybinds::{Key, KeyCombo};
use crate::layers::overlay::{OverlayPosition, OverlayRequest};
use crate::node::Node;
use crate::widgets::events::{EventResult, WidgetEvent, WidgetEventKind, WidgetEvents};
use crate::widgets::traits::{AnyWidget, RenderContext};

use super::render;
use super::Autocomplete;

impl WidgetEvents for Autocomplete {
    fn on_key(&self, key: &KeyCombo, cx: &AppContext) -> EventResult {
        // Ignore keys with ctrl/alt modifiers
        if key.modifiers.ctrl || key.modifiers.alt {
            return EventResult::Ignored;
        }

        let old_value = self.value();

        if !self.is_open() {
            // Closed state
            match key.key {
                Key::Enter | Key::Char(' ') | Key::Down => {
                    self.open();
                    EventResult::Consumed
                }
                Key::Char(c) => {
                    // Start typing - insert char and open dropdown
                    self.insert_char(c);
                    self.open();
                    let new_value = self.value();
                    cx.set_input_text(new_value.clone());
                    if new_value != old_value {
                        cx.push_event(WidgetEvent::new(WidgetEventKind::Change, self.id_string()));
                    }
                    EventResult::Consumed
                }
                Key::Backspace => {
                    self.delete_char_before();
                    let new_value = self.value();
                    cx.set_input_text(new_value.clone());
                    if new_value != old_value {
                        cx.push_event(WidgetEvent::new(WidgetEventKind::Change, self.id_string()));
                    }
                    EventResult::Consumed
                }
                Key::Delete => {
                    self.delete_char_at();
                    let new_value = self.value();
                    cx.set_input_text(new_value.clone());
                    if new_value != old_value {
                        cx.push_event(WidgetEvent::new(WidgetEventKind::Change, self.id_string()));
                    }
                    EventResult::Consumed
                }
                Key::Left => {
                    self.text_cursor_left();
                    EventResult::Consumed
                }
                Key::Right => {
                    self.text_cursor_right();
                    EventResult::Consumed
                }
                Key::Home => {
                    self.text_cursor_home();
                    EventResult::Consumed
                }
                Key::End => {
                    self.text_cursor_end();
                    EventResult::Consumed
                }
                _ => EventResult::Ignored,
            }
        } else {
            // Open state - navigate dropdown while still allowing text input
            match key.key {
                Key::Up => {
                    self.cursor_up();
                    EventResult::Consumed
                }
                Key::Down => {
                    self.cursor_down();
                    EventResult::Consumed
                }
                Key::Enter => {
                    // Select current cursor position if there are matches
                    if self.filtered_count() > 0 {
                        self.select_at_cursor();
                        let new_value = self.value();
                        cx.set_input_text(new_value.clone());
                        cx.push_event(WidgetEvent::new(WidgetEventKind::Select, self.id_string()));
                    }
                    EventResult::Consumed
                }
                Key::Escape => {
                    self.close();
                    EventResult::Consumed
                }
                Key::Char(c) => {
                    // Continue typing while dropdown is open
                    self.insert_char(c);
                    let new_value = self.value();
                    cx.set_input_text(new_value.clone());
                    if new_value != old_value {
                        cx.push_event(WidgetEvent::new(WidgetEventKind::Change, self.id_string()));
                    }
                    EventResult::Consumed
                }
                Key::Backspace => {
                    self.delete_char_before();
                    let new_value = self.value();
                    cx.set_input_text(new_value.clone());
                    if new_value != old_value {
                        cx.push_event(WidgetEvent::new(WidgetEventKind::Change, self.id_string()));
                    }
                    EventResult::Consumed
                }
                Key::Delete => {
                    self.delete_char_at();
                    let new_value = self.value();
                    cx.set_input_text(new_value.clone());
                    if new_value != old_value {
                        cx.push_event(WidgetEvent::new(WidgetEventKind::Change, self.id_string()));
                    }
                    EventResult::Consumed
                }
                Key::Left => {
                    self.text_cursor_left();
                    EventResult::Consumed
                }
                Key::Right => {
                    self.text_cursor_right();
                    EventResult::Consumed
                }
                Key::Home => {
                    self.text_cursor_home();
                    EventResult::Consumed
                }
                Key::End => {
                    self.text_cursor_end();
                    EventResult::Consumed
                }
                _ => EventResult::Ignored,
            }
        }
    }
}

impl AnyWidget for Autocomplete {
    fn id(&self) -> String {
        self.id_string()
    }

    fn is_dirty(&self) -> bool {
        Autocomplete::is_dirty(self)
    }

    fn clear_dirty(&self) {
        Autocomplete::clear_dirty(self)
    }

    fn is_focusable(&self) -> bool {
        true
    }

    fn captures_input(&self) -> bool {
        true // Autocomplete captures keyboard input when focused
    }

    fn intrinsic_height(&self) -> u16 {
        1 // Trigger is one line tall
    }

    fn hides_children_from_layout(&self) -> bool {
        true // Children are for options, not layout
    }

    fn intrinsic_width(&self) -> u16 {
        // Width based on value or placeholder, plus dropdown indicator
        let value = self.value();
        let placeholder = self.placeholder();
        let content = if value.is_empty() { &placeholder } else { &value };
        (content.len() + 2).max(15) as u16
    }

    fn dispatch_click(&self, _x: u16, _y: u16, _cx: &AppContext) -> EventResult {
        // Click toggles the dropdown
        self.toggle();
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

        // y coordinate maps to filtered item index
        let index = y as usize;

        if index < self.filtered_count() {
            // Set cursor and select
            self.set_cursor(index);
            self.select_at_cursor();
            let new_value = self.value();
            cx.set_input_text(new_value);
            cx.push_event(WidgetEvent::new(WidgetEventKind::Select, self.id_string()));
            EventResult::Consumed
        } else {
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

        let index = y as usize;

        if index < self.filtered_count() {
            self.set_cursor(index);
            EventResult::Consumed
        } else {
            EventResult::Ignored
        }
    }

    fn render(&self, frame: &mut Frame, area: Rect, focused: bool, ctx: &mut RenderContext<'_>) {
        // Store anchor for overlay positioning
        self.set_anchor_rect(area);

        // Extract option labels from children for filtering
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

        self.set_option_labels(labels);

        // Render the trigger (input with dropdown indicator)
        render::render_trigger(frame, area, self, focused, ctx);

        // If open and has filtered items, register overlay
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
