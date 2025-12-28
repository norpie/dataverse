use crate::element::{Content, Element};
use crate::layout::LayoutResult;

/// Find the deepest clickable element at the given coordinates.
/// Returns None if no clickable element contains the point.
pub fn hit_test(layout: &LayoutResult, root: &Element, x: u16, y: u16) -> Option<String> {
    hit_test_element(layout, root, x, y)
}

/// Find any element (clickable or not) at the given coordinates.
/// Returns the deepest element containing the point.
pub fn hit_test_any(layout: &LayoutResult, root: &Element, x: u16, y: u16) -> Option<String> {
    hit_test_element_any(layout, root, x, y)
}

fn hit_test_element(layout: &LayoutResult, element: &Element, x: u16, y: u16) -> Option<String> {
    let rect = layout.get(&element.id)?;

    if !rect.contains(x, y) {
        return None;
    }

    // Check children in reverse order (last rendered = on top)
    if let Content::Children(children) = &element.content {
        for child in children.iter().rev() {
            if let Some(id) = hit_test_element(layout, child, x, y) {
                return Some(id);
            }
        }
    }

    // Return this element if clickable
    if element.clickable {
        Some(element.id.clone())
    } else {
        None
    }
}

fn hit_test_element_any(layout: &LayoutResult, element: &Element, x: u16, y: u16) -> Option<String> {
    let rect = layout.get(&element.id)?;

    if !rect.contains(x, y) {
        return None;
    }

    // Check children in reverse order (last rendered = on top)
    if let Content::Children(children) = &element.content {
        for child in children.iter().rev() {
            if let Some(id) = hit_test_element_any(layout, child, x, y) {
                return Some(id);
            }
        }
    }

    // Return this element (regardless of clickable status)
    Some(element.id.clone())
}

/// Find the focusable element at the given coordinates.
/// Returns None if no focusable element contains the point.
pub fn hit_test_focusable(
    layout: &LayoutResult,
    root: &Element,
    x: u16,
    y: u16,
) -> Option<String> {
    hit_test_element_focusable(layout, root, x, y)
}

fn hit_test_element_focusable(
    layout: &LayoutResult,
    element: &Element,
    x: u16,
    y: u16,
) -> Option<String> {
    let rect = layout.get(&element.id)?;

    if !rect.contains(x, y) {
        return None;
    }

    // Check children in reverse order (last rendered = on top)
    if let Content::Children(children) = &element.content {
        for child in children.iter().rev() {
            if let Some(id) = hit_test_element_focusable(layout, child, x, y) {
                return Some(id);
            }
        }
    }

    // Return this element if focusable
    if element.focusable {
        Some(element.id.clone())
    } else {
        None
    }
}
