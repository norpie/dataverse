use std::collections::HashMap;

use super::Rect;
use crate::element::{Content, Element};
use crate::text::display_width;
use crate::types::{Align, Direction, Position, Size};

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

    // Apply margin - shrink available space and offset position
    let margin = &element.margin;
    let after_margin = available.shrink(margin.top, margin.right, margin.bottom, margin.left);

    // Calculate this element's size within margin-adjusted space
    let width = resolve_size(element.width, after_margin.width, element, true);
    let height = resolve_size(element.height, after_margin.height, element, false);
    let rect = Rect::new(after_margin.x, after_margin.y, width, height);
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

    // Separate flow children from absolute children
    let flow_children: Vec<_> = children
        .iter()
        .filter(|c| c.position != Position::Absolute)
        .collect();
    let absolute_children: Vec<_> = children
        .iter()
        .filter(|c| c.position == Position::Absolute)
        .collect();

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

    // First pass: calculate fixed sizes and count flex items (flow children only)
    let mut fixed_total = 0u16;
    let mut flex_count = 0u16;
    let gap_total = element.gap * flow_children.len().saturating_sub(1) as u16;

    for child in &flow_children {
        // Account for child's margin in main axis
        let child_margin_main = if is_row {
            child.margin.left + child.margin.right
        } else {
            child.margin.top + child.margin.bottom
        };

        let child_main_size = if is_row { child.width } else { child.height };
        match child_main_size {
            Size::Fixed(n) => fixed_total += n + child_margin_main,
            Size::Auto => {
                // For auto, estimate based on content
                let estimated = estimate_size(child, is_row);
                fixed_total += estimated + child_margin_main;
            }
            Size::Fill | Size::Flex(_) => flex_count += 1,
            Size::Percent(p) => fixed_total += (main_size as f32 * p) as u16 + child_margin_main,
        }
    }

    // Calculate remaining space for flex items
    let remaining = main_size.saturating_sub(fixed_total + gap_total);
    let flex_size = if flex_count > 0 {
        remaining / flex_count
    } else {
        0
    };

    // Calculate child sizes first (including margins)
    let mut child_sizes: Vec<(u16, u16, u16)> = Vec::with_capacity(flow_children.len()); // (main, margin_before, margin_after)
    let mut total_child_size = 0u16;

    for child in &flow_children {
        let (margin_before, margin_after) = if is_row {
            (child.margin.left, child.margin.right)
        } else {
            (child.margin.top, child.margin.bottom)
        };

        let child_main_size = if is_row { child.width } else { child.height };

        let main = match child_main_size {
            Size::Fixed(n) => n,
            Size::Auto => estimate_size(child, is_row),
            Size::Fill | Size::Flex(_) => flex_size,
            Size::Percent(p) => (main_size as f32 * p) as u16,
        };

        // Apply min/max constraints on main axis
        let (min_main, max_main) = if is_row {
            (child.min_width, child.max_width)
        } else {
            (child.min_height, child.max_height)
        };
        let main = min_main.map_or(main, |m| main.max(m));
        let main = max_main.map_or(main, |m| main.min(m));

        child_sizes.push((main, margin_before, margin_after));
        total_child_size += main + margin_before + margin_after;
    }

    // Calculate justify spacing
    let total_with_gaps = total_child_size + gap_total;
    let extra_space = main_size.saturating_sub(total_with_gaps);

    let (start_offset, between_gap) = match element.justify {
        crate::types::Justify::Start => (0, element.gap),
        crate::types::Justify::End => (extra_space, element.gap),
        crate::types::Justify::Center => (extra_space / 2, element.gap),
        crate::types::Justify::SpaceBetween => {
            if flow_children.len() > 1 {
                (0, extra_space / (flow_children.len() - 1) as u16 + element.gap)
            } else {
                (0, element.gap)
            }
        }
        crate::types::Justify::SpaceAround => {
            if flow_children.is_empty() {
                (0, element.gap)
            } else {
                let spacing = extra_space / flow_children.len() as u16;
                (spacing / 2, spacing + element.gap)
            }
        }
    };

    // Second pass: assign rects to flow children with justify
    let mut offset = start_offset;

    for (i, child) in flow_children.iter().enumerate() {
        let (main, margin_before, margin_after) = child_sizes[i];

        // Account for cross-axis margin
        let (cross_margin_before, cross_margin_after) = if is_row {
            (child.margin.top, child.margin.bottom)
        } else {
            (child.margin.left, child.margin.right)
        };

        // Determine alignment for this child (align_self overrides parent's align)
        let child_align = child.align_self.unwrap_or(element.align);

        let child_cross_size = if is_row { child.height } else { child.width };
        let available_cross = cross_size.saturating_sub(cross_margin_before + cross_margin_after);

        let cross = match child_cross_size {
            Size::Fixed(n) => n,
            Size::Fill | Size::Flex(_) => available_cross,
            Size::Auto => {
                if child_align == Align::Stretch {
                    available_cross
                } else {
                    estimate_size(child, !is_row).min(available_cross)
                }
            }
            Size::Percent(p) => (cross_size as f32 * p) as u16,
        };

        // Apply min/max constraints on cross axis
        let (min_cross, max_cross) = if is_row {
            (child.min_height, child.max_height)
        } else {
            (child.min_width, child.max_width)
        };
        let cross = min_cross.map_or(cross, |m| cross.max(m));
        let cross = max_cross.map_or(cross, |m| cross.min(m));

        // Clamp to available space
        let clamped_main = main.min(main_size.saturating_sub(offset + margin_before));
        let clamped_cross = cross.min(available_cross);

        // Calculate cross-axis offset based on alignment
        let cross_offset = match child_align {
            Align::Start => cross_margin_before,
            Align::Center => {
                cross_margin_before + (available_cross.saturating_sub(clamped_cross)) / 2
            }
            Align::End => cross_margin_before + available_cross.saturating_sub(clamped_cross),
            Align::Stretch => cross_margin_before,
        };

        // Apply margin_before to offset
        let child_rect = if is_row {
            Rect::new(
                inner.x + offset + margin_before,
                inner.y + cross_offset,
                clamped_main,
                clamped_cross,
            )
        } else {
            Rect::new(
                inner.x + cross_offset,
                inner.y + offset + margin_before,
                clamped_cross,
                clamped_main,
            )
        };

        // Insert child rect directly (parent has determined dimensions)
        result.insert(child.id.clone(), child_rect);
        // Recurse for grandchildren
        layout_children(child, child_rect, result);

        offset += margin_before + main + margin_after + between_gap;
    }

    // Layout absolute children (they position themselves)
    for child in absolute_children {
        layout_element(child, rect, result);
    }
}

fn resolve_size(size: Size, available: u16, element: &Element, is_width: bool) -> u16 {
    let base = match size {
        Size::Fixed(n) => n.min(available),
        Size::Fill | Size::Flex(_) => available,
        Size::Auto => estimate_size(element, is_width).min(available),
        Size::Percent(p) => ((available as f32 * p) as u16).min(available),
    };

    // Apply min/max constraints
    let (min, max) = if is_width {
        (element.min_width, element.max_width)
    } else {
        (element.min_height, element.max_height)
    };

    let with_min = min.map_or(base, |m| base.max(m));
    let with_max = max.map_or(with_min, |m| with_min.min(m));

    with_max.min(available)
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
                display_width(text) as u16
            } else {
                // Count newlines for height
                text.lines().count().max(1) as u16
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
