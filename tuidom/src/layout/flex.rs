use std::collections::HashMap;

use super::Rect;
use crate::element::{Content, Element};
use crate::types::{Direction, Position, Size};

pub type LayoutResult = HashMap<String, Rect>;

pub fn layout(element: &Element, available: Rect) -> LayoutResult {
    let mut result = LayoutResult::new();
    layout_element(element, available, &mut result);
    result
}

fn layout_element(element: &Element, available: Rect, result: &mut LayoutResult) {
    // Handle absolute positioning
    if element.position == Position::Absolute {
        let x = element.left.unwrap_or(0) as u16;
        let y = element.top.unwrap_or(0) as u16;
        let width = resolve_size(element.width, available.width, element, true);
        let height = resolve_size(element.height, available.height, element, false);
        let rect = Rect::new(x, y, width, height);
        result.insert(element.id.clone(), rect);
        layout_children(element, rect, result);
        return;
    }

    // Calculate this element's size
    let width = resolve_size(element.width, available.width, element, true);
    let height = resolve_size(element.height, available.height, element, false);
    let rect = Rect::new(available.x, available.y, width, height);
    result.insert(element.id.clone(), rect);

    layout_children(element, rect, result);
}

fn layout_children(element: &Element, rect: Rect, result: &mut LayoutResult) {
    let Content::Children(children) = &element.content else {
        return;
    };

    if children.is_empty() {
        return;
    }

    // Account for border
    let border_size = if element.style.border == crate::types::Border::None {
        0
    } else {
        1
    };

    let inner = rect.shrink(
        element.padding.top + border_size,
        element.padding.right + border_size,
        element.padding.bottom + border_size,
        element.padding.left + border_size,
    );

    let is_row = element.direction == Direction::Row;
    let main_size = if is_row { inner.width } else { inner.height };
    let cross_size = if is_row { inner.height } else { inner.width };

    // First pass: calculate fixed sizes and count flex items
    let mut fixed_total = 0u16;
    let mut flex_count = 0u16;
    let gap_total = element.gap * (children.len().saturating_sub(1)) as u16;

    for child in children {
        let child_main_size = if is_row { child.width } else { child.height };
        match child_main_size {
            Size::Fixed(n) => fixed_total += n,
            Size::Auto => {
                // For auto, estimate based on content
                let estimated = estimate_size(child, is_row);
                fixed_total += estimated;
            }
            Size::Fill | Size::Flex(_) => flex_count += 1,
            Size::Percent(p) => fixed_total += (main_size as f32 * p) as u16,
        }
    }

    // Calculate remaining space for flex items
    let remaining = main_size.saturating_sub(fixed_total + gap_total);
    let flex_size = if flex_count > 0 {
        remaining / flex_count
    } else {
        0
    };

    // Calculate child sizes first
    let mut child_sizes: Vec<u16> = Vec::with_capacity(children.len());
    let mut total_child_size = 0u16;

    for child in children {
        let child_main_size = if is_row { child.width } else { child.height };

        let main = match child_main_size {
            Size::Fixed(n) => n,
            Size::Auto => estimate_size(child, is_row),
            Size::Fill | Size::Flex(_) => flex_size,
            Size::Percent(p) => (main_size as f32 * p) as u16,
        };

        child_sizes.push(main);
        total_child_size += main;
    }

    // Calculate justify spacing
    let total_with_gaps = total_child_size + gap_total;
    let extra_space = main_size.saturating_sub(total_with_gaps);

    let (start_offset, between_gap) = match element.justify {
        crate::types::Justify::Start => (0, element.gap),
        crate::types::Justify::End => (extra_space, element.gap),
        crate::types::Justify::Center => (extra_space / 2, element.gap),
        crate::types::Justify::SpaceBetween => {
            if children.len() > 1 {
                (0, extra_space / (children.len() - 1) as u16 + element.gap)
            } else {
                (0, element.gap)
            }
        }
        crate::types::Justify::SpaceAround => {
            let spacing = extra_space / children.len() as u16;
            (spacing / 2, spacing + element.gap)
        }
    };

    // Second pass: assign rects with justify
    let mut offset = start_offset;

    for (i, child) in children.iter().enumerate() {
        let child_cross_size = if is_row { child.height } else { child.width };
        let main = child_sizes[i];

        let cross = match child_cross_size {
            Size::Fixed(n) => n,
            Size::Fill | Size::Flex(_) | Size::Auto => cross_size,
            Size::Percent(p) => (cross_size as f32 * p) as u16,
        };

        // Clamp to available space
        let clamped_main = main.min(main_size.saturating_sub(offset));
        let clamped_cross = cross.min(cross_size);

        let child_rect = if is_row {
            Rect::new(inner.x + offset, inner.y, clamped_main, clamped_cross)
        } else {
            Rect::new(inner.x, inner.y + offset, clamped_cross, clamped_main)
        };

        layout_element(child, child_rect, result);
        offset += main + between_gap;
    }
}

fn resolve_size(size: Size, available: u16, element: &Element, is_width: bool) -> u16 {
    match size {
        Size::Fixed(n) => n.min(available),
        Size::Fill | Size::Flex(_) => available,
        Size::Auto => estimate_size(element, is_width).min(available),
        Size::Percent(p) => ((available as f32 * p) as u16).min(available),
    }
}

fn estimate_size(element: &Element, is_width: bool) -> u16 {
    let border_size = if element.style.border == crate::types::Border::None {
        0
    } else {
        2
    };
    let padding = if is_width {
        element.padding.horizontal_total()
    } else {
        element.padding.vertical_total()
    };

    let content_size = match &element.content {
        Content::Text(text) => {
            if is_width {
                text.len() as u16
            } else {
                1
            }
        }
        Content::Children(children) => {
            if children.is_empty() {
                0
            } else if element.direction == Direction::Row && is_width
                || element.direction == Direction::Column && !is_width
            {
                // Sum along main axis
                let gap_total = element.gap * (children.len().saturating_sub(1)) as u16;
                children
                    .iter()
                    .map(|c| estimate_size(c, is_width))
                    .sum::<u16>()
                    + gap_total
            } else {
                // Max along cross axis
                children
                    .iter()
                    .map(|c| estimate_size(c, is_width))
                    .max()
                    .unwrap_or(0)
            }
        }
        Content::None => 0,
        Content::Custom(_) => 10, // arbitrary default
    };

    content_size + padding + border_size
}
