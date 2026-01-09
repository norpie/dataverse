use std::collections::HashMap;

use super::Rect;
use crate::animation::AnimationState;
use crate::element::{Content, Element};
use crate::text::display_width;
use crate::types::{Align, Direction, Overflow, Position, Size, Wrap};

// =============================================================================
// Virtualization Constants
// =============================================================================

/// Minimum number of children before virtualization kicks in.
/// Below this threshold, the overhead of visibility calculation isn't worth it.
const VIRTUALIZATION_THRESHOLD: usize = 20;

/// Number of extra items to layout above/below visible area.
/// Provides buffer for smooth scrolling without visual pop-in.
const VIRTUALIZATION_BUFFER: usize = 5;

// =============================================================================
// Scroll Context for Virtualization
// =============================================================================

/// Scroll context passed down from scrollable ancestors.
/// Allows children to virtualize even if they're not scrollable themselves.
#[derive(Debug, Clone, Copy)]
struct ScrollContext {
    /// Vertical scroll offset from the scrollable ancestor.
    scroll_y: u16,
    /// Viewport height of the scrollable ancestor.
    viewport_height: u16,
}

/// Layout results containing element rects and content sizes.
#[derive(Debug, Default, Clone)]
pub struct LayoutResult {
    rects: HashMap<String, Rect>,
    /// Content and viewport sizes for scrollable elements
    /// (content_width, content_height, viewport_width, viewport_height)
    content_sizes: HashMap<String, (u16, u16, u16, u16)>,
}

impl LayoutResult {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, id: &str) -> Option<&Rect> {
        self.rects.get(id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut Rect> {
        self.rects.get_mut(id)
    }

    pub fn insert(&mut self, id: String, rect: Rect) {
        self.rects.insert(id, rect);
    }

    /// Get the content size for a scrollable element.
    /// Returns (content_width, content_height) if the element has overflow != Visible.
    pub fn content_size(&self, id: &str) -> Option<(u16, u16)> {
        self.content_sizes.get(id).map(|(cw, ch, _, _)| (*cw, *ch))
    }

    /// Get the viewport (inner) size for a scrollable element.
    /// Returns (viewport_width, viewport_height) if the element has overflow != Visible.
    pub fn viewport_size(&self, id: &str) -> Option<(u16, u16)> {
        self.content_sizes.get(id).map(|(_, _, vw, vh)| (*vw, *vh))
    }

    pub fn set_content_size(
        &mut self,
        id: String,
        content_width: u16,
        content_height: u16,
        viewport_width: u16,
        viewport_height: u16,
    ) {
        self.content_sizes.insert(
            id,
            (
                content_width,
                content_height,
                viewport_width,
                viewport_height,
            ),
        );
    }

    /// Iterate over all element rects.
    pub fn iter_rects(&self) -> impl Iterator<Item = (&String, &Rect)> {
        self.rects.iter()
    }
}

pub fn layout(element: &Element, available: Rect, animation: &AnimationState) -> LayoutResult {
    let mut result = LayoutResult::new();
    layout_element(element, available, &mut result, animation, None);
    result
}

fn layout_element(
    element: &Element,
    available: Rect,
    result: &mut LayoutResult,
    animation: &AnimationState,
    scroll_ctx: Option<ScrollContext>,
) {
    // Handle absolute positioning
    if element.position == Position::Absolute {
        let rect = layout_absolute(element, available);
        result.insert(element.id.clone(), rect);
        layout_children(element, rect, result, animation, scroll_ctx);
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
    layout_children(element, rect, result, animation, scroll_ctx);
}

/// Layout an absolutely positioned element within its containing block.
/// Supports left/top and right/bottom anchoring, including stretching when both are specified.
fn layout_absolute(element: &Element, container: Rect) -> Rect {
    // Determine width - absolute elements can overflow container, so use unclamped
    let width = match (element.left, element.right) {
        // Both specified: stretch to fill between anchors
        (Some(left), Some(right)) => {
            let left_u = left.max(0) as u16;
            let right_u = right.max(0) as u16;
            container.width.saturating_sub(left_u + right_u)
        }
        // Only explicit size or default - use unclamped to allow overflow
        _ => resolve_size_clamped(element.width, container.width, element, true, false),
    };

    // Determine height - absolute elements can overflow container, so use unclamped
    let height = match (element.top, element.bottom) {
        // Both specified: stretch to fill between anchors
        (Some(top), Some(bottom)) => {
            let top_u = top.max(0) as u16;
            let bottom_u = bottom.max(0) as u16;
            container.height.saturating_sub(top_u + bottom_u)
        }
        // Only explicit size or default - use unclamped to allow overflow
        _ => resolve_size_clamped(element.height, container.height, element, false, false),
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

fn layout_children(
    element: &Element,
    rect: Rect,
    result: &mut LayoutResult,
    animation: &AnimationState,
    scroll_ctx: Option<ScrollContext>,
) {
    // Handle Frames content - only layout current frame
    if let Content::Frames { children, .. } = &element.content {
        let frame_idx = animation.current_frame(&element.id);
        if let Some(frame) = children.get(frame_idx) {
            // Layout the current frame as a single child filling the container
            layout_element(frame, rect, result, animation, scroll_ctx);
        }
        return;
    }

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

    // Compute inner rect without scrollbar reservation first
    let inner_no_scroll = rect.shrink(
        element.padding.top + border_size,
        element.padding.right + border_size,
        element.padding.bottom + border_size,
        element.padding.left + border_size,
    );

    // Determine scrollbar reservations based on overflow mode
    let is_row = element.direction == Direction::Row;
    let (scrollbar_right, scrollbar_bottom) = match element.overflow {
        Overflow::Scroll => (1, 1), // Always show both scrollbars
        Overflow::Auto => {
            // Smart reservation: only reserve space for scrollbars that are needed
            // Note: compute_content_size calls estimate_size which has O(1) fast path
            // when child elements have item_height set
            let (content_width, content_height) =
                compute_content_size(&flow_children, inner_no_scroll, is_row, element.gap);

            let needs_vertical = content_height > inner_no_scroll.height;
            // Check horizontal need accounting for vertical scrollbar taking 1 char
            let available_width_after_vscroll = if needs_vertical {
                inner_no_scroll.width.saturating_sub(1)
            } else {
                inner_no_scroll.width
            };
            let needs_horizontal = content_width > available_width_after_vscroll;

            (
                if needs_vertical { 1 } else { 0 },
                if needs_horizontal { 1 } else { 0 },
            )
        }
        _ => (0, 0),
    };

    let inner = rect.shrink(
        element.padding.top + border_size,
        element.padding.right + border_size + scrollbar_right,
        element.padding.bottom + border_size + scrollbar_bottom,
        element.padding.left + border_size,
    );

    let (scroll_x, scroll_y) = element.scroll_offset;

    // Determine scroll context for children:
    // - If this element is scrollable, create new scroll context
    // - Otherwise, inherit from parent
    let is_scrollable = element.overflow == Overflow::Scroll || element.overflow == Overflow::Auto;
    let child_scroll_ctx = if is_scrollable {
        Some(ScrollContext {
            scroll_y,
            viewport_height: inner.height,
        })
    } else {
        scroll_ctx
    };

    // Check if we should use virtualized layout
    // Can virtualize if: this element is scrollable OR has scroll context from ancestor
    let effective_scroll_ctx = if is_scrollable {
        Some(ScrollContext {
            scroll_y,
            viewport_height: inner.height,
        })
    } else {
        scroll_ctx
    };

    if should_virtualize(element, flow_children.len(), effective_scroll_ctx) {
        let ctx = effective_scroll_ctx.unwrap(); // Safe: should_virtualize ensures this

        // Virtualized path: only fully layout visible children
        // Returns content size calculated during position estimation
        let (content_width, content_height) = layout_children_virtualized(
            element,
            &flow_children,
            inner,
            ctx.scroll_y,
            ctx.viewport_height,
            element.gap,
            result,
            animation,
            child_scroll_ctx,
        );

        // Set content size for scroll calculations (only if this element is scrollable)
        if is_scrollable {
            result.set_content_size(
                element.id.clone(),
                content_width,
                content_height,
                inner.width,
                inner.height,
            );
        }

        // Apply scroll offset to visible children only (others don't have rects)
        if is_scrollable && (scroll_x > 0 || scroll_y > 0) {
            for child in &flow_children {
                apply_scroll_offset_recursive(child, scroll_x, scroll_y, result);
            }
        }

        // Layout absolute children (not virtualized)
        for child in &absolute_children {
            layout_element(child, rect, result, animation, child_scroll_ctx);
        }

        if is_scrollable && (scroll_x > 0 || scroll_y > 0) {
            for child in &absolute_children {
                apply_scroll_offset_recursive(child, scroll_x, scroll_y, result);
            }
        }

        return;
    }

    // Non-virtualized path (original code)
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
            animation,
            child_scroll_ctx,
        );
        cross_offset += line_cross_size + element.gap;
    }

    // Store content size for scrollable elements using natural/unconstrained child sizes
    if element.overflow == Overflow::Scroll || element.overflow == Overflow::Auto {
        // Calculate natural content size (what children would take if unconstrained)
        let (content_width, content_height) =
            compute_content_size(&flow_children, inner, is_row, element.gap);
        result.set_content_size(
            element.id.clone(),
            content_width,
            content_height,
            inner.width,
            inner.height,
        );
    }

    // Apply scroll offset to all flow children
    if scroll_x > 0 || scroll_y > 0 {
        for child in &flow_children {
            apply_scroll_offset_recursive(child, scroll_x, scroll_y, result);
        }
    }

    // Layout absolute children (they position themselves relative to container)
    for child in &absolute_children {
        layout_element(child, rect, result, animation, child_scroll_ctx);
    }

    // Apply scroll offset to absolute children too (so dropdowns follow their anchors)
    if scroll_x > 0 || scroll_y > 0 {
        for child in &absolute_children {
            apply_scroll_offset_recursive(child, scroll_x, scroll_y, result);
        }
    }
}

/// Compute the natural content size from children's intrinsic sizes.
/// This calculates what size the content WOULD be if unconstrained,
/// which is needed for scroll containers to know the scrollable extent.
fn compute_content_size(
    children: &[&Element],
    inner: Rect,
    is_row: bool,
    gap: u16,
) -> (u16, u16) {
    if children.is_empty() {
        return (inner.width, inner.height);
    }

    // For scrollable content, we need the natural/unconstrained size
    // Sum up children sizes along main axis, max along cross axis
    let mut main_total = 0u16;
    let mut cross_max = 0u16;

    for (i, child) in children.iter().enumerate() {
        let child_main = estimate_size(child, is_row);
        let child_cross = estimate_size(child, !is_row);

        // Add margins
        let (main_margin, cross_margin) = if is_row {
            (
                child.margin.left + child.margin.right,
                child.margin.top + child.margin.bottom,
            )
        } else {
            (
                child.margin.top + child.margin.bottom,
                child.margin.left + child.margin.right,
            )
        };

        main_total += child_main + main_margin;
        if i > 0 {
            main_total += gap;
        }
        cross_max = cross_max.max(child_cross + cross_margin);
    }

    // Return (width, height) based on direction
    if is_row {
        (main_total, cross_max)
    } else {
        (cross_max, main_total)
    }
}

/// Apply scroll offset to an element and all its descendants.
/// Only recurses into children that are present in the layout result
/// (virtualized off-screen elements won't have their children laid out).
fn apply_scroll_offset_recursive(
    element: &Element,
    scroll_x: u16,
    scroll_y: u16,
    result: &mut LayoutResult,
) {
    if let Some(rect) = result.get_mut(&element.id) {
        rect.x = rect.x.saturating_sub(scroll_x);
        rect.y = rect.y.saturating_sub(scroll_y);
    }

    if let Content::Children(children) = &element.content {
        for child in children {
            // Only recurse if child was laid out (virtualized off-screen children
            // have a rect but their descendants don't)
            if result.get(&child.id).is_some() {
                apply_scroll_offset_recursive(child, scroll_x, scroll_y, result);
            }
        }
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
        let with_gap = if lines.last().is_none_or(|l| l.is_empty()) {
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
        Size::Fill => 0,       // Will be distributed via flex
        Size::Flex(_) => 0,    // Will be distributed via flex
        Size::Percent(_) => 0, // Needs container size, treat as flex for wrapping
    }
}

/// Layout a single line of flex items, returns the line's cross-axis size
#[allow(clippy::too_many_arguments)]
fn layout_line(
    line: &[&Element],
    parent: &Element,
    inner: Rect,
    cross_offset: u16,
    main_size: u16,
    cross_size: u16,
    is_row: bool,
    result: &mut LayoutResult,
    animation: &AnimationState,
    scroll_ctx: Option<ScrollContext>,
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
        let cross_available =
            available_cross.saturating_sub(cross_margin_before + cross_margin_after);

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
        layout_children(child, child_rect, result, animation, scroll_ctx);

        main_offset += margin_before + main + margin_after + between_gap;
    }

    line_cross_size
}

/// Resolve a size value to a concrete pixel value.
/// If `clamp` is true, the result is clamped to available space.
/// Absolute positioned elements should use clamp=false to allow overflow.
fn resolve_size_clamped(
    size: Size,
    available: u16,
    element: &Element,
    is_width: bool,
    clamp: bool,
) -> u16 {
    let base = match size {
        Size::Fixed(n) => {
            if clamp {
                n.min(available)
            } else {
                n
            }
        }
        Size::Fill | Size::Flex(_) => available,
        Size::Auto => {
            let est = estimate_size(element, is_width);
            if clamp {
                est.min(available)
            } else {
                est
            }
        }
        Size::Percent(p) => {
            let pct = (available as f32 * p) as u16;
            if clamp {
                pct.min(available)
            } else {
                pct
            }
        }
    };

    // Apply min/max constraints
    let (min, max) = if is_width {
        (element.min_width, element.max_width)
    } else {
        (element.min_height, element.max_height)
    };

    let with_min = min.map_or(base, |m| base.max(m));
    let with_max = max.map_or(with_min, |m| with_min.min(m));

    if clamp {
        with_max.min(available)
    } else {
        with_max
    }
}

fn resolve_size(size: Size, available: u16, element: &Element, is_width: bool) -> u16 {
    resolve_size_clamped(size, available, element, is_width, true)
}

fn estimate_size(element: &Element, is_width: bool) -> u16 {
    // Check for explicit Fixed size first - this takes precedence over content estimation
    let explicit_size = if is_width {
        element.width
    } else {
        element.height
    };
    if let Size::Fixed(n) = explicit_size {
        return n;
    }

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

    // Reserve space for scrollbars in scrollable containers
    let scrollbar_size = match element.overflow {
        Overflow::Scroll | Overflow::Auto => 1,
        _ => 0,
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
                // O(1) fast path: use item_height hint for column height estimation
                if !is_width && element.direction == Direction::Column {
                    if let Some(item_height) = element.item_height {
                        let child_count = children.len();
                        let stride = item_height.saturating_add(element.gap) as u32;
                        return padding
                            + border_size
                            + scrollbar_size
                            + (child_count as u32 * stride).saturating_sub(element.gap as u32)
                                as u16;
                    }
                }
                // O(n) fallback
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
        Content::Frames { children, .. } => {
            // Size to fit the largest frame
            children
                .iter()
                .map(|c| estimate_size(c, is_width))
                .max()
                .unwrap_or(0)
        }
        Content::TextInput {
            value, placeholder, ..
        } => {
            let text = if value.is_empty() {
                placeholder.as_deref().unwrap_or("")
            } else {
                value.as_str()
            };
            if is_width {
                // Add 1 for cursor at end
                display_width(text) as u16 + 1
            } else {
                1 // Single line input
            }
        }
    };

    content_size + padding + border_size + scrollbar_size
}

// =============================================================================
// Virtualization
// =============================================================================

/// Check if a container should use virtualized layout.
/// Returns true for column containers with enough children that have scroll context
/// (either from this element being scrollable, or inherited from scrollable ancestor).
fn should_virtualize(
    element: &Element,
    child_count: usize,
    scroll_ctx: Option<ScrollContext>,
) -> bool {
    let has_scroll_context = scroll_ctx.is_some();
    let is_column = element.direction == Direction::Column;
    let no_wrap = element.wrap == Wrap::NoWrap;
    let enough_children = child_count >= VIRTUALIZATION_THRESHOLD;

    has_scroll_context && is_column && no_wrap && enough_children
}

/// Calculate which children are visible given scroll position and viewport.
/// Returns (first_visible_index, last_visible_index, total_content_height).
/// Includes VIRTUALIZATION_BUFFER items above and below.
///
/// When `fixed_item_height` is Some, uses O(1) calculation.
/// Otherwise falls back to O(n) position estimation.
fn compute_visible_range(
    children: &[&Element],
    scroll_y: u16,
    viewport_height: u16,
    gap: u16,
    fixed_item_height: Option<u16>,
) -> (usize, usize, u16) {
    if children.is_empty() {
        return (0, 0, 0);
    }

    let child_count = children.len();

    // O(1) path: fixed item height known
    if let Some(item_height) = fixed_item_height {
        let item_stride = item_height.saturating_add(gap) as u32;
        let total_height = if child_count > 0 {
            // total = n * item_height + (n-1) * gap = n * stride - gap
            (child_count as u32 * item_stride).saturating_sub(gap as u32) as u16
        } else {
            0
        };

        if item_stride == 0 {
            return (0, child_count, total_height);
        }

        // First visible: first child whose bottom edge is past scroll_y
        // Child i has bottom at (i+1) * stride - gap
        // We want (i+1) * stride - gap > scroll_y
        // i+1 > (scroll_y + gap) / stride
        // i > (scroll_y + gap) / stride - 1
        let first_visible = if scroll_y == 0 {
            0
        } else {
            (scroll_y as u32 / item_stride) as usize
        };

        // Last visible: first child whose top edge is past viewport bottom
        // Child i has top at i * stride
        // We want i * stride >= viewport_bottom
        let viewport_bottom = scroll_y.saturating_add(viewport_height) as u32;
        let last_visible = ((viewport_bottom + item_stride - 1) / item_stride) as usize;

        // Apply buffer
        let start = first_visible.saturating_sub(VIRTUALIZATION_BUFFER);
        let end = (last_visible + VIRTUALIZATION_BUFFER).min(child_count);

        log::debug!(
            "[virtualize-O1] children={} item_height={} scroll_y={} viewport={} -> visible={}..{}",
            child_count,
            item_height,
            scroll_y,
            viewport_height,
            start,
            end
        );

        return (start, end, total_height);
    }

    // O(n) fallback: estimate each child's height
    let mut offset = 0u16;
    let mut first_visible = 0;
    let mut last_visible = child_count;
    let viewport_bottom = scroll_y.saturating_add(viewport_height);
    let mut found_first = false;
    let mut found_last = false;

    for (i, child) in children.iter().enumerate() {
        let height = estimate_child_height(child);
        let bottom = offset.saturating_add(height);

        if !found_first && bottom > scroll_y {
            first_visible = i;
            found_first = true;
        }

        if !found_last && offset >= viewport_bottom {
            last_visible = i;
            found_last = true;
            // Can't break early - need total height
        }

        offset = bottom;
        if i < child_count - 1 {
            offset = offset.saturating_add(gap);
        }
    }

    let total_height = offset;

    // Apply buffer
    let start = first_visible.saturating_sub(VIRTUALIZATION_BUFFER);
    let end = (last_visible + VIRTUALIZATION_BUFFER).min(child_count);

    (start, end, total_height)
}

/// Estimate a child's height including its margins.
fn estimate_child_height(child: &Element) -> u16 {
    let base_height = estimate_size(child, false); // false = height
    let margin_height = child.margin.top + child.margin.bottom;
    base_height + margin_height
}

/// Layout children with virtualization - only fully layout visible items.
/// Off-screen items are skipped entirely (no rects inserted).
/// Returns (content_width, content_height) for scroll calculations.
#[allow(clippy::too_many_arguments)]
fn layout_children_virtualized(
    element: &Element,
    flow_children: &[&Element],
    inner: Rect,
    scroll_y: u16,
    viewport_height: u16,
    gap: u16,
    result: &mut LayoutResult,
    animation: &AnimationState,
    scroll_ctx: Option<ScrollContext>,
) -> (u16, u16) {
    let fixed_item_height = element.item_height;
    let (visible_start, visible_end, total_height) =
        compute_visible_range(flow_children, scroll_y, viewport_height, gap, fixed_item_height);

    log::debug!(
        "[virtualize] id={} children={} visible={}..{} scroll_y={} viewport={} content_height={} fixed_height={:?}",
        element.id,
        flow_children.len(),
        visible_start,
        visible_end,
        scroll_y,
        viewport_height,
        total_height,
        fixed_item_height
    );

    // Layout ONLY visible children - skip off-screen entirely
    for i in visible_start..visible_end {
        let child = flow_children[i];

        // Calculate y offset for this child
        let y_offset = if let Some(item_height) = fixed_item_height {
            // O(1): fixed stride
            let stride = item_height.saturating_add(gap);
            (i as u16).saturating_mul(stride)
        } else {
            // O(n) fallback: sum up heights of preceding children
            // This is only called for visible children, so bounded by visible count
            let mut offset = 0u16;
            for j in 0..i {
                offset = offset
                    .saturating_add(estimate_child_height(flow_children[j]))
                    .saturating_add(gap);
            }
            offset
        };

        let child_height = fixed_item_height.unwrap_or_else(|| {
            estimate_child_height(child).saturating_sub(child.margin.top + child.margin.bottom)
        });

        // Calculate child rect
        let child_x = inner.x + child.margin.left;
        let child_y = inner.y + y_offset + child.margin.top;
        let child_width = inner.width.saturating_sub(child.margin.left + child.margin.right);

        let child_rect = Rect::new(child_x, child_y, child_width, child_height);
        result.insert(child.id.clone(), child_rect);
        layout_children(child, child_rect, result, animation, scroll_ctx);
    }

    // Content width is just the inner width for column layout
    (inner.width, total_height)
}
