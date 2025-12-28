use std::collections::HashMap;

use super::Rect;
use crate::element::{Content, Element};
use crate::text::display_width;
use crate::types::{Align, Direction, Position, Size, Wrap};

pub type LayoutResult = HashMap<String, Rect>;

pub fn layout(element: &Element, available: Rect) -> LayoutResult {
    let mut result = LayoutResult::new();
    layout_element(element, available, &mut result);
    result
}

fn layout_element(element: &Element, available: Rect, result: &mut LayoutResult) {
    // Handle absolute positioning
    if element.position == Position::Absolute {
        let rect = layout_absolute(element, available);
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
    let mut rect = Rect::new(after_margin.x, after_margin.y, width, height);

    // Handle relative positioning - offset from normal flow position
    if element.position == Position::Relative {
        if let Some(left) = element.left {
            rect.x = (rect.x as i16 + left).max(0) as u16;
        }
        if let Some(right) = element.right {
            rect.x = (rect.x as i16 - right).max(0) as u16;
        }
        if let Some(top) = element.top {
            rect.y = (rect.y as i16 + top).max(0) as u16;
        }
        if let Some(bottom) = element.bottom {
            rect.y = (rect.y as i16 - bottom).max(0) as u16;
        }
    }

    result.insert(element.id.clone(), rect);
    layout_children(element, rect, result);
}

/// Layout an absolutely positioned element within its containing block.
/// Supports left/top and right/bottom anchoring, including stretching when both are specified.
fn layout_absolute(element: &Element, container: Rect) -> Rect {
    // Determine width
    let width = match (element.left, element.right) {
        // Both specified: stretch to fill between anchors
        (Some(left), Some(right)) => {
            let left_u = left.max(0) as u16;
            let right_u = right.max(0) as u16;
            container.width.saturating_sub(left_u + right_u)
        }
        // Only explicit size or default
        _ => resolve_size(element.width, container.width, element, true),
    };

    // Determine height
    let height = match (element.top, element.bottom) {
        // Both specified: stretch to fill between anchors
        (Some(top), Some(bottom)) => {
            let top_u = top.max(0) as u16;
            let bottom_u = bottom.max(0) as u16;
            container.height.saturating_sub(top_u + bottom_u)
        }
        // Only explicit size or default
        _ => resolve_size(element.height, container.height, element, false),
    };

    // Determine x position
    let x = match (element.left, element.right) {
        (Some(left), _) => container.x.saturating_add_signed(left),
        (None, Some(right)) => {
            // Anchor to right edge
            (container.right() as i16 - width as i16 - right).max(0) as u16
        }
        (None, None) => container.x,
    };

    // Determine y position
    let y = match (element.top, element.bottom) {
        (Some(top), _) => container.y.saturating_add_signed(top),
        (None, Some(bottom)) => {
            // Anchor to bottom edge
            (container.bottom() as i16 - height as i16 - bottom).max(0) as u16
        }
        (None, None) => container.y,
    };

    Rect::new(x, y, width, height)
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

    // Split children into lines (for wrapping)
    let lines = if element.wrap == Wrap::Wrap {
        split_into_lines(&flow_children, main_size, element.gap, is_row)
    } else {
        vec![flow_children.clone()]
    };

    // Layout each line
    let mut cross_offset = 0u16;
    for line in &lines {
        let line_cross_size = layout_line(
            line,
            element,
            inner,
            cross_offset,
            main_size,
            cross_size,
            is_row,
            result,
        );
        cross_offset += line_cross_size + element.gap;
    }

    // Layout absolute children (they position themselves)
    for child in absolute_children {
        layout_element(child, rect, result);
    }
}

/// Split children into lines based on available main axis space
fn split_into_lines<'a>(
    children: &[&'a Element],
    main_size: u16,
    gap: u16,
    is_row: bool,
) -> Vec<Vec<&'a Element>> {
    let mut lines: Vec<Vec<&Element>> = vec![vec![]];
    let mut current_line_size = 0u16;

    for child in children {
        let child_main = get_base_main_size(child, is_row);
        let child_margin = if is_row {
            child.margin.left + child.margin.right
        } else {
            child.margin.top + child.margin.bottom
        };
        let child_total = child_main + child_margin;

        // Add gap if not first item in line
        let with_gap = if lines.last().map_or(true, |l| l.is_empty()) {
            child_total
        } else {
            child_total + gap
        };

        // Check if we need to wrap
        if current_line_size + with_gap > main_size && !lines.last().unwrap().is_empty() {
            lines.push(vec![]);
            current_line_size = child_total;
        } else {
            current_line_size += with_gap;
        }

        lines.last_mut().unwrap().push(child);
    }

    lines
}

/// Get the base main axis size for a child (before flex grow/shrink)
fn get_base_main_size(child: &Element, is_row: bool) -> u16 {
    let size = if is_row { child.width } else { child.height };
    match size {
        Size::Fixed(n) => n,
        Size::Auto => estimate_size(child, is_row),
        Size::Fill => 0, // Will be distributed via flex
        Size::Flex(_) => 0, // Will be distributed via flex
        Size::Percent(_) => 0, // Needs container size, treat as flex for wrapping
    }
}

/// Layout a single line of flex items, returns the line's cross-axis size
fn layout_line(
    line: &[&Element],
    parent: &Element,
    inner: Rect,
    cross_offset: u16,
    main_size: u16,
    cross_size: u16,
    is_row: bool,
    result: &mut LayoutResult,
) -> u16 {
    if line.is_empty() {
        return 0;
    }

    let gap_total = parent.gap * line.len().saturating_sub(1) as u16;

    // First pass: calculate base sizes and collect flex info
    let mut base_sizes: Vec<u16> = Vec::with_capacity(line.len());
    let mut margins: Vec<(u16, u16)> = Vec::with_capacity(line.len());
    let mut total_base = 0u16;
    let mut total_flex_grow = 0u16;
    let mut total_flex_shrink = 0u16;

    for child in line {
        let (margin_before, margin_after) = if is_row {
            (child.margin.left, child.margin.right)
        } else {
            (child.margin.top, child.margin.bottom)
        };
        margins.push((margin_before, margin_after));

        let child_main_size = if is_row { child.width } else { child.height };
        let (base, flex_grow) = match child_main_size {
            Size::Fixed(n) => (n, child.flex_grow),
            Size::Auto => (estimate_size(child, is_row), child.flex_grow),
            Size::Fill => (0, 1.max(child.flex_grow)), // Fill acts as flex_grow: 1
            Size::Flex(n) => (0, n.max(child.flex_grow)), // Flex(n) acts as flex_grow: n
            Size::Percent(p) => ((main_size as f32 * p) as u16, child.flex_grow),
        };

        base_sizes.push(base);
        total_base += base + margin_before + margin_after;
        total_flex_grow += flex_grow;
        total_flex_shrink += child.flex_shrink;
    }

    // Calculate remaining space (positive = grow, negative = shrink)
    let total_with_gaps = total_base + gap_total;
    let remaining = main_size as i32 - total_with_gaps as i32;

    // Second pass: apply flex grow or shrink
    let mut final_sizes: Vec<u16> = Vec::with_capacity(line.len());

    for (i, child) in line.iter().enumerate() {
        let base = base_sizes[i];
        let child_main_size = if is_row { child.width } else { child.height };

        let flex_grow = match child_main_size {
            Size::Fill => 1.max(child.flex_grow),
            Size::Flex(n) => n.max(child.flex_grow),
            _ => child.flex_grow,
        };

        let adjusted = if remaining > 0 && total_flex_grow > 0 {
            // Grow: distribute extra space proportionally
            let grow_amount = (remaining as u32 * flex_grow as u32 / total_flex_grow as u32) as u16;
            base + grow_amount
        } else if remaining < 0 && total_flex_shrink > 0 {
            // Shrink: reduce size proportionally
            let shrink_amount =
                ((-remaining) as u32 * child.flex_shrink as u32 / total_flex_shrink as u32) as u16;
            base.saturating_sub(shrink_amount)
        } else {
            base
        };

        // Apply min/max constraints
        let (min_main, max_main) = if is_row {
            (child.min_width, child.max_width)
        } else {
            (child.min_height, child.max_height)
        };
        let constrained = min_main.map_or(adjusted, |m| adjusted.max(m));
        let constrained = max_main.map_or(constrained, |m| constrained.min(m));

        final_sizes.push(constrained);
    }

    // Recalculate total for justify spacing
    let mut total_final = 0u16;
    for (i, &size) in final_sizes.iter().enumerate() {
        total_final += size + margins[i].0 + margins[i].1;
    }
    let total_with_gaps = total_final + gap_total;
    let extra_space = main_size.saturating_sub(total_with_gaps);

    let (start_offset, between_gap) = match parent.justify {
        crate::types::Justify::Start => (0, parent.gap),
        crate::types::Justify::End => (extra_space, parent.gap),
        crate::types::Justify::Center => (extra_space / 2, parent.gap),
        crate::types::Justify::SpaceBetween => {
            if line.len() > 1 {
                (0, extra_space / (line.len() - 1) as u16 + parent.gap)
            } else {
                (0, parent.gap)
            }
        }
        crate::types::Justify::SpaceAround => {
            if line.is_empty() {
                (0, parent.gap)
            } else {
                let spacing = extra_space / line.len() as u16;
                (spacing / 2, spacing + parent.gap)
            }
        }
    };

    // Third pass: position children and calculate line cross size
    let mut main_offset = start_offset;
    let mut line_cross_size = 0u16;
    let available_cross = cross_size.saturating_sub(cross_offset);

    for (i, child) in line.iter().enumerate() {
        let main = final_sizes[i];
        let (margin_before, margin_after) = margins[i];

        // Cross-axis margin
        let (cross_margin_before, cross_margin_after) = if is_row {
            (child.margin.top, child.margin.bottom)
        } else {
            (child.margin.left, child.margin.right)
        };

        let child_align = child.align_self.unwrap_or(parent.align);
        let child_cross_size = if is_row { child.height } else { child.width };
        let cross_available = available_cross.saturating_sub(cross_margin_before + cross_margin_after);

        let cross = match child_cross_size {
            Size::Fixed(n) => n.min(cross_available),
            Size::Fill | Size::Flex(_) => cross_available,
            Size::Auto => {
                if child_align == Align::Stretch {
                    cross_available
                } else {
                    estimate_size(child, !is_row).min(cross_available)
                }
            }
            Size::Percent(p) => ((available_cross as f32 * p) as u16).min(cross_available),
        };

        // Apply min/max on cross axis
        let (min_cross, max_cross) = if is_row {
            (child.min_height, child.max_height)
        } else {
            (child.min_width, child.max_width)
        };
        let cross = min_cross.map_or(cross, |m| cross.max(m));
        let cross = max_cross.map_or(cross, |m| cross.min(m));

        line_cross_size = line_cross_size.max(cross + cross_margin_before + cross_margin_after);

        // Calculate cross-axis position
        let child_cross_offset = match child_align {
            Align::Start => cross_margin_before,
            Align::Center => cross_margin_before + (cross_available.saturating_sub(cross)) / 2,
            Align::End => cross_margin_before + cross_available.saturating_sub(cross),
            Align::Stretch => cross_margin_before,
        };

        let mut child_rect = if is_row {
            Rect::new(
                inner.x + main_offset + margin_before,
                inner.y + cross_offset + child_cross_offset,
                main,
                cross,
            )
        } else {
            Rect::new(
                inner.x + cross_offset + child_cross_offset,
                inner.y + main_offset + margin_before,
                cross,
                main,
            )
        };

        // Apply relative positioning
        if child.position == Position::Relative {
            if let Some(left) = child.left {
                child_rect.x = (child_rect.x as i16 + left).max(0) as u16;
            }
            if let Some(right) = child.right {
                child_rect.x = (child_rect.x as i16 - right).max(0) as u16;
            }
            if let Some(top) = child.top {
                child_rect.y = (child_rect.y as i16 + top).max(0) as u16;
            }
            if let Some(bottom) = child.bottom {
                child_rect.y = (child_rect.y as i16 - bottom).max(0) as u16;
            }
        }

        result.insert(child.id.clone(), child_rect);
        layout_children(child, child_rect, result);

        main_offset += margin_before + main + margin_after + between_gap;
    }

    line_cross_size
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
