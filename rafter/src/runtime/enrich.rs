//! Element enrichment with runtime state.
//!
//! This module provides the `enrich_elements` function which populates
//! elements with runtime state before rendering:
//! - Sets `focused` flag based on FocusState
//! - Computes effective style (merging base + focused/disabled styles)
//! - Inherits foreground color from parent if not explicitly set
//! - Populates text input cursor/selection from TextInputState
//! - Sets scroll_offset from ScrollState

use tuidom::{Color, Content, Element, FocusState, Overflow, ScrollState, TextInputState};

/// Enrich elements with runtime state before rendering.
///
/// This step:
/// 1. Sets `focused` flag based on FocusState
/// 2. Computes effective style (merging base + focused/disabled styles)
/// 3. Inherits foreground color from parent if not explicitly set
/// 4. Populates text input cursor/selection from TextInputState
/// 5. Sets scroll_offset from ScrollState
pub fn enrich_elements(
    element: &mut Element,
    focus: &FocusState,
    text_inputs: &TextInputState,
    scroll: &ScrollState,
) {
    enrich_elements_inner(element, focus, text_inputs, scroll, None);
}

fn enrich_elements_inner(
    element: &mut Element,
    focus: &FocusState,
    text_inputs: &TextInputState,
    scroll: &ScrollState,
    inherited_foreground: Option<&Color>,
) {
    // 1. Set focused flag for all elements
    element.focused = focus.focused() == Some(element.id.as_str());

    // 2. Compute effective style INTO element.style
    //    This enables animation system to see style changes
    if element.disabled {
        element.style = element.style.merge(&element.style_disabled);
    } else if element.focused {
        element.style = element.style.merge(&element.style_focused);
    }

    // 3. Inherit foreground from parent if not explicitly set
    if element.style.foreground.is_none() {
        if let Some(fg) = inherited_foreground {
            element.style.foreground = Some(fg.clone());
        }
    }

    // 4. For text inputs: populate cursor/selection from TextInputState
    if let Content::TextInput {
        cursor,
        selection,
        focused,
        ..
    } = &mut element.content
    {
        if let Some(data) = text_inputs.get_data(&element.id) {
            *cursor = data.cursor;
            *selection = data.selection();
        }
        *focused = element.focused;
    }

    // 5. Set scroll_offset from ScrollState - per axis based on overflow mode
    // Elements with overflow_x/overflow_y = Scroll/Auto use tuidom's scroll system for that axis
    // Elements with .scrollable(true) handle scrolling via handlers (separate from overflow)
    let offset = scroll.get(&element.id);

    // Apply horizontal scroll offset if overflow_x is Scroll/Auto
    let use_tuidom_scroll_x = element.overflow_x == Overflow::Scroll || element.overflow_x == Overflow::Auto;
    let scroll_x = if use_tuidom_scroll_x { offset.x } else { 0 };

    // Apply vertical scroll offset if overflow_y is Scroll/Auto
    // BUT: if element has .scrollable(true), it handles vertical scroll via handlers instead
    let use_tuidom_scroll_y = (element.overflow_y == Overflow::Scroll || element.overflow_y == Overflow::Auto)
        && !element.scrollable;
    let scroll_y = if use_tuidom_scroll_y { offset.y } else { 0 };

    // Log for debugging
    if use_tuidom_scroll_x {
        log::debug!("[enrich] {} use_tuidom_scroll_x=true offset=({},{}) setting scroll_offset=({},{})",
            element.id, offset.x, offset.y, scroll_x, scroll_y);
    }

    element.scroll_offset = (scroll_x, scroll_y);

    // Verify the scroll_offset was actually set
    if use_tuidom_scroll_x && element.scroll_offset != (scroll_x, scroll_y) {
        log::error!("[enrich] BUG: scroll_offset wasn't set correctly!");
    }

    // 6. Recurse into children, passing this element's foreground for inheritance
    let child_foreground = element.style.foreground.as_ref();
    match &mut element.content {
        Content::Children(children) => {
            for child in children {
                enrich_elements_inner(child, focus, text_inputs, scroll, child_foreground);
            }
        }
        Content::Frames { children, .. } => {
            for child in children {
                enrich_elements_inner(child, focus, text_inputs, scroll, child_foreground);
            }
        }
        _ => {}
    }
}
