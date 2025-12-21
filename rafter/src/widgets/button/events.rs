//! Event handling for the Button widget.

use ratatui::layout::Rect;
use ratatui::Frame;

use crate::context::AppContext;
use crate::keybinds::{Key, KeyCombo};
use crate::widgets::events::EventResult;
use crate::widgets::traits::AnyWidget;

use super::Button;

impl AnyWidget for Button {
    fn id(&self) -> String {
        self.id_string()
    }

    fn is_dirty(&self) -> bool {
        Button::is_dirty(self)
    }

    fn clear_dirty(&self) {
        Button::clear_dirty(self)
    }

    fn is_focusable(&self) -> bool {
        true
    }

    fn captures_input(&self) -> bool {
        false
    }

    fn dispatch_click(&self, _x: u16, _y: u16, _cx: &AppContext) -> EventResult {
        // Button click is handled by runtime dispatching on_click handler
        // We just signal that we consumed the click
        EventResult::Consumed
    }

    fn dispatch_key(&self, key: &KeyCombo, _cx: &AppContext) -> EventResult {
        // Only handle keys without modifiers
        if key.modifiers.ctrl || key.modifiers.alt || key.modifiers.shift {
            return EventResult::Ignored;
        }

        match key.key {
            Key::Enter | Key::Char(' ') => {
                // Enter/Space activates the button - runtime will dispatch on_activate handler
                EventResult::Consumed
            }
            _ => EventResult::Ignored,
        }
    }

    fn render(&self, frame: &mut Frame, area: Rect, focused: bool) {
        super::render::render_button(frame, &self.label(), focused, area);
    }
}
