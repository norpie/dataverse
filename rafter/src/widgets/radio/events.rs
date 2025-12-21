//! Event handling for the RadioGroup widget.

use ratatui::layout::Rect;
use ratatui::Frame;

use crate::context::AppContext;
use crate::keybinds::{Key, KeyCombo};
use crate::widgets::events::{EventResult, WidgetEvents};
use crate::widgets::traits::{AnyWidget, RenderContext};

use super::RadioGroup;

impl WidgetEvents for RadioGroup {
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

impl AnyWidget for RadioGroup {
    fn id(&self) -> String {
        self.id_string()
    }

    fn is_dirty(&self) -> bool {
        RadioGroup::is_dirty(self)
    }

    fn clear_dirty(&self) {
        RadioGroup::clear_dirty(self)
    }

    fn is_focusable(&self) -> bool {
        true
    }

    fn captures_input(&self) -> bool {
        false
    }

    fn dispatch_click(&self, _x: u16, y: u16, _cx: &AppContext) -> EventResult {
        // Select the clicked option based on y position
        let index = y as usize;
        if index < self.len() {
            self.select(index);
        }
        EventResult::Consumed
    }

    fn dispatch_key(&self, key: &KeyCombo, cx: &AppContext) -> EventResult {
        self.on_key(key, cx)
    }

    fn render(&self, frame: &mut Frame, area: Rect, focused: bool, _ctx: &mut RenderContext<'_>) {
        // When focused, highlight the selected option (or first if none selected)
        let focused_index = if focused {
            Some(self.selected().unwrap_or(0))
        } else {
            None
        };

        super::render::render_radio_group(
            frame,
            &self.options(),
            self.selected(),
            self.selected_char(),
            self.unselected_char(),
            ratatui::style::Style::default(),
            focused,
            focused_index,
            area,
        );
    }
}
