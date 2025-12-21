//! Event handling for the Checkbox widget.

use ratatui::layout::Rect;
use ratatui::Frame;

use crate::context::AppContext;
use crate::keybinds::{Key, KeyCombo};
use crate::widgets::events::{EventResult, WidgetEvents};
use crate::widgets::traits::{AnyWidget, RenderContext};

use super::Checkbox;

impl WidgetEvents for Checkbox {
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

impl AnyWidget for Checkbox {
    fn id(&self) -> String {
        self.id_string()
    }

    fn is_dirty(&self) -> bool {
        Checkbox::is_dirty(self)
    }

    fn clear_dirty(&self) {
        Checkbox::clear_dirty(self)
    }

    fn is_focusable(&self) -> bool {
        true
    }

    fn captures_input(&self) -> bool {
        false
    }

    fn dispatch_click(&self, _x: u16, _y: u16, _cx: &AppContext) -> EventResult {
        // Toggle on click
        self.toggle();
        EventResult::Consumed
    }

    fn dispatch_key(&self, key: &KeyCombo, cx: &AppContext) -> EventResult {
        // Delegate to existing WidgetEvents implementation
        self.on_key(key, cx)
    }

    fn render(&self, frame: &mut Frame, area: Rect, focused: bool, _ctx: &mut RenderContext<'_>) {
        super::render::render_checkbox(
            frame,
            self.is_checked(),
            &self.label(),
            self.checked_char(),
            self.unchecked_char(),
            ratatui::style::Style::default(),
            focused,
            area,
        );
    }
}
