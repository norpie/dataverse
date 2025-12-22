//! Event handling for the Button widget.

use ratatui::layout::Rect;
use ratatui::Frame;

use crate::context::AppContext;
use crate::input::keybinds::{Key, KeyCombo};
use crate::widgets::events::{EventResult, WidgetEvent, WidgetEventKind};
use crate::widgets::traits::{AnyWidget, RenderContext};

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

    fn intrinsic_height(&self) -> u16 {
        1
    }

    fn intrinsic_width(&self) -> u16 {
        // Width is " label " = label + 2 (padding)
        (self.label().len() + 2) as u16
    }

    fn dispatch_click(&self, _x: u16, _y: u16, cx: &AppContext) -> EventResult {
        // Push an Activate event so the runtime dispatches the on_click handler
        cx.push_event(WidgetEvent::new(WidgetEventKind::Activate, self.id_string()));
        EventResult::Consumed
    }

    fn dispatch_key(&self, key: &KeyCombo, cx: &AppContext) -> EventResult {
        // Only handle keys without modifiers
        if key.modifiers.ctrl || key.modifiers.alt || key.modifiers.shift {
            return EventResult::Ignored;
        }

        match key.key {
            Key::Enter | Key::Char(' ') => {
                // Push an Activate event so the runtime dispatches the on_click handler
                cx.push_event(WidgetEvent::new(WidgetEventKind::Activate, self.id_string()));
                EventResult::Consumed
            }
            _ => EventResult::Ignored,
        }
    }

    fn render(&self, frame: &mut Frame, area: Rect, focused: bool, _ctx: &mut RenderContext<'_>) {
        super::render::render_button(frame, &self.label(), focused, area);
    }
}
