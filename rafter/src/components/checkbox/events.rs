//! Event handling for the Checkbox component.

use crate::components::events::{ComponentEvents, EventResult};
use crate::context::AppContext;
use crate::keybinds::{Key, KeyCombo};

use super::Checkbox;

impl ComponentEvents for Checkbox {
    fn on_key(&self, key: &KeyCombo, _cx: &AppContext) -> EventResult {
        // Only handle keys without modifiers
        if key.modifiers.ctrl || key.modifiers.alt || key.modifiers.shift {
            return EventResult::Ignored;
        }

        match key.key {
            Key::Char(' ') | Key::Enter => {
                self.toggle();
                EventResult::Consumed
            }
            _ => EventResult::Ignored,
        }
    }
}
