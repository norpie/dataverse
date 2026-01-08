//! Element enrichment with runtime state.
//!
//! This module provides the `enrich_elements` function which populates
//! elements with runtime state before rendering:
//! - Sets `focused` flag based on FocusState
//! - Computes effective style (merging base + focused/disabled styles)
//! - Populates text input cursor/selection from TextInputState
//! - Sets scroll_offset from ScrollState

use tuidom::{Content, Element, FocusState, ScrollState, TextInputState};

/// Enrich elements with runtime state before rendering.
///
/// This step:
/// 1. Sets `focused` flag based on FocusState
/// 2. Computes effective style (merging base + focused/disabled styles)
/// 3. Populates text input cursor/selection from TextInputState
/// 4. Sets scroll_offset from ScrollState
pub fn enrich_elements(
    element: &mut Element,
    focus: &FocusState,
    text_inputs: &TextInputState,
    scroll: &ScrollState,
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

    // 3. For text inputs: populate cursor/selection from TextInputState
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

    // 4. Set scroll_offset from ScrollState
    let offset = scroll.get(&element.id);
    element.scroll_offset = (offset.x, offset.y);

    // 5. Recurse into children
    match &mut element.content {
        Content::Children(children) => {
            for child in children {
                enrich_elements(child, focus, text_inputs, scroll);
            }
        }
        Content::Frames { children, .. } => {
            for child in children {
                enrich_elements(child, focus, text_inputs, scroll);
            }
        }
        _ => {}
    }
}
