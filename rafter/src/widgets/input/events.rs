//! Event handling for the Input widget.

use ratatui::layout::Rect;
use ratatui::Frame;

use crate::context::AppContext;
use crate::keybinds::{Key, KeyCombo};
use crate::widgets::events::{EventResult, WidgetEvents};
use crate::widgets::traits::AnyWidget;

use super::Input;

impl WidgetEvents for Input {
    fn on_key(&self, key: &KeyCombo, cx: &AppContext) -> EventResult {
        // Only handle keys without modifiers (except Shift) or minimal shortcuts
        if key.modifiers.ctrl || key.modifiers.alt {
            return EventResult::Ignored;
        }

        let result = match key.key {
            Key::Backspace => {
                self.delete_char_before();
                EventResult::Consumed
            }
            Key::Delete => {
                self.delete_char_at();
                EventResult::Consumed
            }
            Key::Left => {
                self.cursor_left();
                EventResult::Consumed
            }
            Key::Right => {
                self.cursor_right();
                EventResult::Consumed
            }
            Key::Home => {
                self.cursor_home();
                EventResult::Consumed
            }
            Key::End => {
                self.cursor_end();
                EventResult::Consumed
            }
            Key::Char(c) => {
                self.insert_char(c);
                EventResult::Consumed
            }
            _ => EventResult::Ignored,
        };

        // If we consumed the event, update the context with current input value
        if result == EventResult::Consumed {
            cx.set_input_text(self.value());
        }

        result
    }
}

impl AnyWidget for Input {
    fn id(&self) -> String {
        self.id_string()
    }

    fn is_dirty(&self) -> bool {
        Input::is_dirty(self)
    }

    fn clear_dirty(&self) {
        Input::clear_dirty(self)
    }

    fn is_focusable(&self) -> bool {
        true
    }

    fn captures_input(&self) -> bool {
        true // Input captures keyboard input when focused
    }

    fn dispatch_click(&self, _x: u16, _y: u16, _cx: &AppContext) -> EventResult {
        // Click focuses the input - runtime handles focus
        EventResult::Consumed
    }

    fn dispatch_key(&self, key: &KeyCombo, cx: &AppContext) -> EventResult {
        self.on_key(key, cx)
    }

    fn render(&self, frame: &mut Frame, area: Rect, focused: bool) {
        super::render::render_input(
            frame,
            &self.value(),
            &self.placeholder(),
            self.cursor(),
            ratatui::style::Style::default(),
            focused,
            area,
        );
    }
}
