//! Event handling for the RadioGroup component.

use crate::components::events::{ComponentEvents, EventResult};
use crate::context::AppContext;
use crate::keybinds::{Key, KeyCombo};

use super::RadioGroup;

impl ComponentEvents for RadioGroup {
    fn on_key(&self, key: &KeyCombo, _cx: &AppContext) -> EventResult {
        // Only handle keys without modifiers
        if key.modifiers.ctrl || key.modifiers.alt || key.modifiers.shift {
            return EventResult::Ignored;
        }

        let len = self.len();
        if len == 0 {
            return EventResult::Ignored;
        }

        match key.key {
            Key::Char(' ') | Key::Enter => {
                // Space/Enter confirms current selection (no-op if already selected)
                // The runtime handles dispatching on_change
                EventResult::Consumed
            }
            Key::Up | Key::Char('k') => {
                // Move selection up (with wrap)
                let current = self.selected().unwrap_or(0);
                let new_index = if current == 0 { len - 1 } else { current - 1 };
                self.select(new_index);
                EventResult::Consumed
            }
            Key::Down | Key::Char('j') => {
                // Move selection down (with wrap)
                let current = self.selected().unwrap_or(0);
                let new_index = if current + 1 >= len { 0 } else { current + 1 };
                self.select(new_index);
                EventResult::Consumed
            }
            Key::Home => {
                // Select first option
                self.select(0);
                EventResult::Consumed
            }
            Key::End => {
                // Select last option
                self.select(len - 1);
                EventResult::Consumed
            }
            _ => EventResult::Ignored,
        }
    }
}
