//! Shared scrollbar event handling.
//!
//! These helper functions can be used by any component that implements
//! `ScrollbarState` to handle scrollbar interactions consistently.

use super::{ScrollbarDrag, ScrollbarState};
use crate::components::events::EventResult;
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
    component: &S,
    x: u16,
    y: u16,
    _cx: &AppContext,
) -> Option<EventResult> {
    // Check vertical scrollbar
    if let Some(geom) = component.vertical_scrollbar()
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
            component.scroll_to_ratio(None, Some(ratio));
            grab_offset
        };

        component.set_drag(Some(ScrollbarDrag {
            is_vertical: true,
            grab_offset,
        }));
        return Some(EventResult::StartDrag);
    }

    // Check horizontal scrollbar
    if let Some(geom) = component.horizontal_scrollbar()
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
            component.scroll_to_ratio(Some(ratio), None);
            grab_offset
        };

        component.set_drag(Some(ScrollbarDrag {
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
    component: &S,
    direction: ScrollDirection,
    amount: u16,
    _cx: &AppContext,
) -> EventResult {
    let amount = amount as i16;
    match direction {
        ScrollDirection::Up => component.scroll_by(0, -amount),
        ScrollDirection::Down => component.scroll_by(0, amount),
        ScrollDirection::Left => component.scroll_by(-amount, 0),
        ScrollDirection::Right => component.scroll_by(amount, 0),
    }
    EventResult::Consumed
}

/// Handle a drag event on scrollbars.
///
/// Updates scroll position while dragging a scrollbar handle.
/// Returns `EventResult::Consumed` if a drag was active, `EventResult::Ignored` otherwise.
pub fn handle_scrollbar_drag<S: ScrollbarState>(
    component: &S,
    x: u16,
    y: u16,
    _modifiers: Modifiers,
    _cx: &AppContext,
) -> EventResult {
    if let Some(drag) = component.drag() {
        if drag.is_vertical {
            if let Some(geom) = component.vertical_scrollbar() {
                let ratio = geom.position_to_ratio_with_offset(x, y, true, drag.grab_offset);
                component.scroll_to_ratio(None, Some(ratio));
            }
        } else if let Some(geom) = component.horizontal_scrollbar() {
            let ratio = geom.position_to_ratio_with_offset(x, y, false, drag.grab_offset);
            component.scroll_to_ratio(Some(ratio), None);
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
pub fn handle_scrollbar_release<S: ScrollbarState>(
    component: &S,
    _cx: &AppContext,
) -> EventResult {
    if component.drag().is_some() {
        component.set_drag(None);
        EventResult::Consumed
    } else {
        EventResult::Ignored
    }
}
