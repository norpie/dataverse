//! Shared scrollbar event handling.
//!
//! These helper functions can be used by any widget that implements
//! `ScrollbarState` to handle scrollbar interactions consistently.

use super::{ScrollbarDrag, ScrollbarState};
use crate::widgets::events::EventResult;
use crate::context::AppContext;
use crate::events::{Modifiers, ScrollDirection};

/// Handle a click event on scrollbars.
///
/// Returns `Some(EventResult::StartDrag)` if the click was on a scrollbar,
/// or `None` if the click was not on any scrollbar.
///
/// This handles both vertical and horizontal scrollbars, including:
/// - Clicking on the handle to start a drag
/// - Clicking on the track to jump to that position
pub fn handle_scrollbar_click<S: ScrollbarState>(
    widget: &S,
    x: u16,
    y: u16,
    _cx: &AppContext,
) -> Option<EventResult> {
    // Check vertical scrollbar
    if let Some(geom) = widget.vertical_scrollbar()
        && geom.contains(x, y)
    {
        let grab_offset = if geom.handle_contains(x, y, true) {
            // Clicked on handle - remember offset within handle
            y.saturating_sub(geom.y + geom.handle_pos)
        } else {
            // Clicked on track - calculate proportional offset and jump
            let track_ratio = (y.saturating_sub(geom.y) as f32) / (geom.height.max(1) as f32);
            let grab_offset = (track_ratio * geom.handle_size as f32) as u16;
            let ratio = geom.position_to_ratio_with_offset(x, y, true, grab_offset);
            widget.scroll_to_ratio(None, Some(ratio));
            grab_offset
        };

        widget.set_drag(Some(ScrollbarDrag {
            is_vertical: true,
            grab_offset,
        }));
        return Some(EventResult::StartDrag);
    }

    // Check horizontal scrollbar
    if let Some(geom) = widget.horizontal_scrollbar()
        && geom.contains(x, y)
    {
        let grab_offset = if geom.handle_contains(x, y, false) {
            // Clicked on handle - remember offset within handle
            x.saturating_sub(geom.x + geom.handle_pos)
        } else {
            // Clicked on track - calculate proportional offset and jump
            let track_ratio = (x.saturating_sub(geom.x) as f32) / (geom.width.max(1) as f32);
            let grab_offset = (track_ratio * geom.handle_size as f32) as u16;
            let ratio = geom.position_to_ratio_with_offset(x, y, false, grab_offset);
            widget.scroll_to_ratio(Some(ratio), None);
            grab_offset
        };

        widget.set_drag(Some(ScrollbarDrag {
            is_vertical: false,
            grab_offset,
        }));
        return Some(EventResult::StartDrag);
    }

    None
}

/// Handle a scroll wheel event.
///
/// Scrolls the content in the appropriate direction.
/// Returns `EventResult::Consumed` if scrolling was handled.
pub fn handle_scroll<S: ScrollbarState>(
    widget: &S,
    direction: ScrollDirection,
    amount: u16,
    _cx: &AppContext,
) -> EventResult {
    let amount = amount as i16;
    match direction {
        ScrollDirection::Up => widget.scroll_by(0, -amount),
        ScrollDirection::Down => widget.scroll_by(0, amount),
        ScrollDirection::Left => widget.scroll_by(-amount, 0),
        ScrollDirection::Right => widget.scroll_by(amount, 0),
    }
    EventResult::Consumed
}

/// Handle a drag event on scrollbars.
///
/// Updates scroll position while dragging a scrollbar handle.
/// Returns `EventResult::Consumed` if a drag was active, `EventResult::Ignored` otherwise.
pub fn handle_scrollbar_drag<S: ScrollbarState>(
    widget: &S,
    x: u16,
    y: u16,
    _modifiers: Modifiers,
    _cx: &AppContext,
) -> EventResult {
    if let Some(drag) = widget.drag() {
        if drag.is_vertical {
            if let Some(geom) = widget.vertical_scrollbar() {
                let ratio = geom.position_to_ratio_with_offset(x, y, true, drag.grab_offset);
                widget.scroll_to_ratio(None, Some(ratio));
            }
        } else if let Some(geom) = widget.horizontal_scrollbar() {
            let ratio = geom.position_to_ratio_with_offset(x, y, false, drag.grab_offset);
            widget.scroll_to_ratio(Some(ratio), None);
        }
        EventResult::Consumed
    } else {
        EventResult::Ignored
    }
}

/// Handle a mouse release event.
///
/// Ends any active scrollbar drag.
/// Returns `EventResult::Consumed` if a drag was active, `EventResult::Ignored` otherwise.
pub fn handle_scrollbar_release<S: ScrollbarState>(widget: &S, _cx: &AppContext) -> EventResult {
    if widget.drag().is_some() {
        widget.set_drag(None);
        EventResult::Consumed
    } else {
        EventResult::Ignored
    }
}
