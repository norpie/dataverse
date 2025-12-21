//! Event handling for the ScrollArea component.

use crate::components::events::{ComponentEvents, EventResult};
use crate::components::scrollbar::{
    handle_scroll, handle_scrollbar_click, handle_scrollbar_drag, handle_scrollbar_release,
    ScrollbarState,
};
use crate::context::AppContext;
use crate::events::{Modifiers, ScrollDirection};
use crate::keybinds::{Key, KeyCombo};

use super::ScrollArea;

/// Lines to scroll per arrow key press.
const SCROLL_LINES: i16 = 1;
/// Lines to scroll per page up/down.
const SCROLL_PAGE_LINES: i16 = 10;

impl ComponentEvents for ScrollArea {
    fn on_click(&self, x: u16, y: u16, cx: &AppContext) -> EventResult {
        // Delegate to shared scrollbar click handling
        handle_scrollbar_click(self, x, y, cx).unwrap_or(EventResult::Ignored)
    }

    fn on_scroll(&self, direction: ScrollDirection, amount: u16, cx: &AppContext) -> EventResult {
        handle_scroll(self, direction, amount, cx)
    }

    fn on_drag(&self, x: u16, y: u16, modifiers: Modifiers, cx: &AppContext) -> EventResult {
        handle_scrollbar_drag(self, x, y, modifiers, cx)
    }

    fn on_release(&self, cx: &AppContext) -> EventResult {
        handle_scrollbar_release(self, cx)
    }

    fn on_key(&self, key: &KeyCombo, _cx: &AppContext) -> EventResult {
        // Ignore keys with ctrl/alt modifiers (let app handle those)
        if key.modifiers.ctrl || key.modifiers.alt {
            return EventResult::Ignored;
        }

        match key.key {
            Key::Up => {
                self.scroll_by(0, -SCROLL_LINES);
                EventResult::Consumed
            }
            Key::Down => {
                self.scroll_by(0, SCROLL_LINES);
                EventResult::Consumed
            }
            Key::Left => {
                self.scroll_by(-SCROLL_LINES, 0);
                EventResult::Consumed
            }
            Key::Right => {
                self.scroll_by(SCROLL_LINES, 0);
                EventResult::Consumed
            }
            Key::PageUp => {
                // Scroll by viewport height, or fallback to SCROLL_PAGE_LINES
                let viewport_height = ScrollbarState::viewport_height(self) as i16;
                let amount = if viewport_height > 0 {
                    viewport_height
                } else {
                    SCROLL_PAGE_LINES
                };
                self.scroll_by(0, -amount);
                EventResult::Consumed
            }
            Key::PageDown => {
                let viewport_height = ScrollbarState::viewport_height(self) as i16;
                let amount = if viewport_height > 0 {
                    viewport_height
                } else {
                    SCROLL_PAGE_LINES
                };
                self.scroll_by(0, amount);
                EventResult::Consumed
            }
            Key::Home => {
                self.scroll_to_top();
                EventResult::Consumed
            }
            Key::End => {
                self.scroll_to_bottom();
                EventResult::Consumed
            }
            _ => EventResult::Ignored,
        }
    }
}
