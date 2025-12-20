//! Event handling for the Scrollable component.

use crate::components::events::{ComponentEvents, EventResult};
use crate::events::ScrollDirection;

use super::Scrollable;

/// Internal drag state for scrollbar dragging.
#[derive(Debug, Clone, Copy)]
pub struct ScrollbarDrag {
    /// Whether dragging the vertical scrollbar (false = horizontal).
    pub is_vertical: bool,
    /// Offset within the handle where the user grabbed.
    pub grab_offset: u16,
}

impl ComponentEvents for Scrollable {
    fn on_click(&self, x: u16, y: u16) -> EventResult {
        // Check vertical scrollbar
        if let Some(geom) = self.vertical_scrollbar() {
            if geom.contains(x, y) {
                let grab_offset = if geom.handle_contains(x, y, true) {
                    // Clicked on handle - remember offset within handle
                    y.saturating_sub(geom.y + geom.handle_pos)
                } else {
                    // Clicked on track - calculate proportional offset and jump
                    let track_ratio =
                        (y.saturating_sub(geom.y) as f32) / (geom.height.max(1) as f32);
                    let grab_offset = (track_ratio * geom.handle_size as f32) as u16;
                    let ratio = geom.position_to_ratio_with_offset(x, y, true, grab_offset);
                    self.scroll_to_ratio(None, Some(ratio));
                    grab_offset
                };

                self.set_drag(Some(ScrollbarDrag {
                    is_vertical: true,
                    grab_offset,
                }));
                return EventResult::StartDrag;
            }
        }

        // Check horizontal scrollbar
        if let Some(geom) = self.horizontal_scrollbar() {
            if geom.contains(x, y) {
                let grab_offset = if geom.handle_contains(x, y, false) {
                    // Clicked on handle - remember offset within handle
                    x.saturating_sub(geom.x + geom.handle_pos)
                } else {
                    // Clicked on track - calculate proportional offset and jump
                    let track_ratio =
                        (x.saturating_sub(geom.x) as f32) / (geom.width.max(1) as f32);
                    let grab_offset = (track_ratio * geom.handle_size as f32) as u16;
                    let ratio = geom.position_to_ratio_with_offset(x, y, false, grab_offset);
                    self.scroll_to_ratio(Some(ratio), None);
                    grab_offset
                };

                self.set_drag(Some(ScrollbarDrag {
                    is_vertical: false,
                    grab_offset,
                }));
                return EventResult::StartDrag;
            }
        }

        EventResult::Ignored
    }

    fn on_scroll(&self, direction: ScrollDirection, amount: u16) -> EventResult {
        let amount = amount as i16;
        match direction {
            ScrollDirection::Up => self.scroll_by(0, -amount),
            ScrollDirection::Down => self.scroll_by(0, amount),
            ScrollDirection::Left => self.scroll_by(-amount, 0),
            ScrollDirection::Right => self.scroll_by(amount, 0),
        }
        EventResult::Consumed
    }

    fn on_drag(&self, x: u16, y: u16) -> EventResult {
        if let Some(drag) = self.drag() {
            if drag.is_vertical {
                if let Some(geom) = self.vertical_scrollbar() {
                    let ratio = geom.position_to_ratio_with_offset(x, y, true, drag.grab_offset);
                    self.scroll_to_ratio(None, Some(ratio));
                }
            } else if let Some(geom) = self.horizontal_scrollbar() {
                let ratio = geom.position_to_ratio_with_offset(x, y, false, drag.grab_offset);
                self.scroll_to_ratio(Some(ratio), None);
            }
            EventResult::Consumed
        } else {
            EventResult::Ignored
        }
    }

    fn on_release(&self) -> EventResult {
        if self.drag().is_some() {
            self.set_drag(None);
            EventResult::Consumed
        } else {
            EventResult::Ignored
        }
    }
}
