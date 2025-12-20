//! Event handling for the Input component.

use crate::components::events::{ComponentEvents, EventResult};
use crate::context::AppContext;
use crate::keybinds::{Key, KeyCombo};

use super::Input;

impl ComponentEvents for Input {
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
