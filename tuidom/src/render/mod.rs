use std::collections::HashMap;
use std::time::Instant;

use crate::animation::{AnimationState, PropertyValue, TransitionProperty};
use crate::buffer::{Buffer, Cell};
use crate::element::{Content, Element};
use crate::layout::{LayoutResult, Rect};
use crate::text::{
    align_offset, char_width, display_width, truncate_to_width, wrap_chars, wrap_words,
};
use crate::types::{Backdrop, Color, ColorContext, ColorKey, Oklch, Overflow, TextWrap};

/// Cache for Color → Oklch conversions during render.
/// Avoids repeated palette conversions for the same colors within a frame.
/// Uses ColorContext to resolve Var and Derived colors before conversion.
struct OklchCache<'a> {
    cache: HashMap<ColorKey, Oklch>,
    color_ctx: &'a ColorContext<'a>,
}

impl<'a> OklchCache<'a> {
    fn new(color_ctx: &'a ColorContext<'a>) -> Self {
        Self {
            cache: HashMap::with_capacity(32),
            color_ctx,
        }
    }

    fn get(&mut self, color: &Color) -> Oklch {
        // First resolve any Var or Derived colors through the theme
        let resolved = self.color_ctx.resolve(color);

        if let Some(key) = resolved.cache_key() {
            *self.cache.entry(key).or_insert_with(|| resolved.to_oklch())
        } else {
            // Shouldn't happen after resolve(), but handle gracefully
            resolved.to_oklch()
        }
    }
}

/// A render item contains an element with its z_index, tree order, and clip rect.
struct RenderItem<'a> {
    element: &'a Element,
    z_index: i16,
    tree_order: usize,
    clip: Option<Rect>,
    /// Cumulative layout_position offset from ancestors (dx, dy).
    /// Children inherit their parent's animation offset.
    layout_offset: (i16, i16),
}

/// Timing stats for render operations (in microseconds).
#[derive(Default)]
struct RenderStats {
    collect_us: f64,
    sort_us: f64,
    background_us: f64,
    border_us: f64,
    text_us: f64,
    scrollbar_us: f64,
    other_us: f64,
    element_count: usize,
}

pub fn render_to_buffer(
    element: &Element,
    layout: &LayoutResult,
    buf: &mut Buffer,
    animation: &AnimationState,
    color_ctx: &ColorContext,
) {
    let t0 = Instant::now();

    // Collect all elements with their effective z_index, tree order, and clip rects
    let mut render_list: Vec<RenderItem> = Vec::new();
    collect_elements(
        element,
        layout,
        &mut render_list,
        0,
        element.z_index,
        None,
        (0, 0), // Initial layout offset
        animation,
    );
    let t1 = Instant::now();

    // Sort by z_index (stable sort preserves tree order for equal z_index)
    render_list.sort_by_key(|item| (item.z_index, item.tree_order));
    let t2 = Instant::now();

    // Track per-operation timing
    let mut stats = RenderStats {
        collect_us: t1.duration_since(t0).as_secs_f64() * 1_000_000.0,
        sort_us: t2.duration_since(t1).as_secs_f64() * 1_000_000.0,
        element_count: render_list.len(),
        ..Default::default()
    };

    // Create color cache for this frame (with theme resolution)
    let mut oklch_cache = OklchCache::new(color_ctx);

    // Render in sorted order
    for item in render_list {
        render_single_element_timed(
            item.element,
            layout,
            buf,
            item.clip,
            item.layout_offset,
            animation,
            &mut stats,
            &mut oklch_cache,
        );
    }

    log::debug!(
        "  render breakdown: collect={:>6.2}µs sort={:>6.2}µs bg={:>6.2}µs border={:>6.2}µs text={:>6.2}µs scrollbar={:>6.2}µs other={:>6.2}µs elements={}",
        stats.collect_us,
        stats.sort_us,
        stats.background_us,
        stats.border_us,
        stats.text_us,
        stats.scrollbar_us,
        stats.other_us,
        stats.element_count,
    );
}

/// Collect all elements in tree order with their effective z_index and clip rects.
/// Children inherit their parent's z_index as a minimum (they render in the same layer or higher).
/// Clip rects are computed based on ancestors with overflow != Visible.
fn collect_elements<'a>(
    element: &'a Element,
    layout: &LayoutResult,
    list: &mut Vec<RenderItem<'a>>,
    tree_order: usize,
    parent_z_index: i16,
    parent_clip: Option<Rect>,
    parent_layout_offset: (i16, i16),
    animation: &AnimationState,
) -> usize {
    let mut order = tree_order;
    // Effective z_index: use element's z_index if explicitly higher, otherwise inherit parent's
    let effective_z = element.z_index.max(parent_z_index);

    // Calculate this element's position animation offset (if any)
    // The unified animation system tracks all position changes (from layout reflow, left/right/top/bottom)
    let now = Instant::now();
    let position_offset = if let Some(layout_rect) = layout.get(&element.id) {
        let (interp_x, interp_y) = animation.get_interpolated_position(&element.id, now);
        let dx = interp_x.map(|x| x as i16 - layout_rect.x as i16).unwrap_or(0);
        let dy = interp_y.map(|y| y as i16 - layout_rect.y as i16).unwrap_or(0);
        (dx, dy)
    } else {
        (0, 0)
    };

    // Cumulative offset: parent's offset + this element's position offset
    let cumulative_offset = (
        parent_layout_offset.0 + position_offset.0,
        parent_layout_offset.1 + position_offset.1,
    );

    // Compute this element's clip rect for its children
    let child_clip = if element.overflow != Overflow::Visible {
        // This element clips its children - compute inner bounds
        if let Some(rect) = layout.get(&element.id) {
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
            // Intersect with parent's clip (if any)
            Some(intersect_rects(inner, parent_clip))
        } else {
            parent_clip
        }
    } else {
        parent_clip
    };

    list.push(RenderItem {
        element,
        z_index: effective_z,
        tree_order: order,
        clip: parent_clip, // This element is clipped by parent's clip
        layout_offset: cumulative_offset,
    });
    order += 1;

    match &element.content {
        Content::Children(children) => {
            for child in children {
                order = collect_elements(
                    child,
                    layout,
                    list,
                    order,
                    effective_z,
                    child_clip,
                    cumulative_offset,
                    animation,
                );
            }
        }
        Content::Frames { children, .. } => {
            // Only collect the current frame
            let frame_idx = animation.current_frame(&element.id);
            if let Some(child) = children.get(frame_idx) {
                order = collect_elements(
                    child,
                    layout,
                    list,
                    order,
                    effective_z,
                    child_clip,
                    cumulative_offset,
                    animation,
                );
            }
        }
        _ => {}
    }

    order
}

/// Intersect two rects, returning the overlapping area.
/// If parent_clip is None, returns rect unchanged.
fn intersect_rects(rect: Rect, parent_clip: Option<Rect>) -> Rect {
    match parent_clip {
        None => rect,
        Some(clip) => {
            let x = rect.x.max(clip.x);
            let y = rect.y.max(clip.y);
            let right = rect.right().min(clip.right());
            let bottom = rect.bottom().min(clip.bottom());
            Rect::new(x, y, right.saturating_sub(x), bottom.saturating_sub(y))
        }
    }
}

/// Render a single element with timing stats collection.
fn render_single_element_timed(
    element: &Element,
    layout: &LayoutResult,
    buf: &mut Buffer,
    clip: Option<Rect>,
    layout_offset: (i16, i16),
    animation: &AnimationState,
    stats: &mut RenderStats,
    oklch_cache: &mut OklchCache,
) {
    let t0 = Instant::now();

    // Apply backdrop BEFORE rendering this element (dims entire buffer)
    apply_backdrop(buf, &element.backdrop);

    let Some(layout_rect) = layout.get(&element.id) else {
        return;
    };

    // Apply cumulative position offset (includes this element's animation + ancestors')
    let rect = if layout_offset != (0, 0) {
        if element.id.starts_with("__toast") {
            log::debug!(
                "[render] {} applying layout_offset ({}, {}) to layout pos ({}, {})",
                element.id, layout_offset.0, layout_offset.1, layout_rect.x, layout_rect.y
            );
        }
        Rect::new(
            (layout_rect.x as i16 + layout_offset.0).max(0) as u16,
            (layout_rect.y as i16 + layout_offset.1).max(0) as u16,
            layout_rect.width,
            layout_rect.height,
        )
    } else {
        *layout_rect
    };

    // If we have a clip rect, intersect with element rect
    let visible_rect = match clip {
        Some(clip_rect) => {
            let clipped = intersect_rects(rect, Some(clip_rect));
            // If completely clipped, skip rendering
            if clipped.width == 0 || clipped.height == 0 {
                return;
            }
            clipped
        }
        None => rect,
    };

    let t1 = Instant::now();
    stats.other_us += t1.duration_since(t0).as_secs_f64() * 1_000_000.0;

    // Get background color (potentially interpolated)
    let background = get_interpolated_color(
        animation,
        &element.id,
        TransitionProperty::Background,
        element.style.background.as_ref(),
    );

    // Render background if set (only within visible area)
    if let Some(bg) = background {
        let oklch = oklch_cache.get(&bg);
        fill_rect(buf, visible_rect, Some(oklch));
    }
    let t2 = Instant::now();
    stats.background_us += t2.duration_since(t1).as_secs_f64() * 1_000_000.0;

    // Render border if set (needs full rect for corners, but clip to visible)
    render_border(element, rect, buf, clip, animation, oklch_cache);
    let t3 = Instant::now();
    stats.border_us += t3.duration_since(t2).as_secs_f64() * 1_000_000.0;

    // Render content (text or custom only, children/frames handled separately via collect_elements)
    match &element.content {
        Content::None | Content::Children(_) | Content::Frames { .. } => {}
        Content::Text(text) => {
            render_text(text, element, rect, buf, clip, animation, oklch_cache);
        }
        Content::TextInput {
            value,
            cursor,
            selection,
            placeholder,
            focused,
        } => {
            render_text_input(
                value,
                *cursor,
                *selection,
                placeholder.as_deref(),
                *focused,
                element,
                rect,
                buf,
                clip,
                animation,
                oklch_cache,
            );
        }
        Content::Custom(custom) => {
            // Custom content gets the full rect; it should handle clipping internally
            custom.render(rect, buf);
        }
    }
    let t4 = Instant::now();
    stats.text_us += t4.duration_since(t3).as_secs_f64() * 1_000_000.0;

    // Render scrollbars for Scroll/Auto overflow
    let content_size = layout.content_size(&element.id);
    let viewport_size = layout.viewport_size(&element.id);
    render_scrollbar(element, rect, buf, clip, content_size, viewport_size);
    let t5 = Instant::now();
    stats.scrollbar_us += t5.duration_since(t4).as_secs_f64() * 1_000_000.0;
}

/// Get interpolated color if there's an active transition, otherwise return the current color.
fn get_interpolated_color(
    animation: &AnimationState,
    element_id: &str,
    property: TransitionProperty,
    current: Option<&Color>,
) -> Option<Color> {
    // Check for active transition
    if let Some(PropertyValue::Color(color)) = animation.get_interpolated(element_id, property) {
        return Some(color);
    }
    // No transition, use current value
    current.cloned()
}

fn fill_rect(buf: &mut Buffer, rect: Rect, bg: Option<Oklch>) {
    let Some(bg_color) = bg else { return }; // Skip if transparent
    for y in rect.y..rect.bottom().min(buf.height()) {
        for x in rect.x..rect.right().min(buf.width()) {
            if let Some(cell) = buf.get_mut(x, y) {
                // Skip if cell already has correct state
                if cell.bg == Some(bg_color) && cell.char == ' ' && !cell.wide_continuation {
                    continue;
                }
                cell.char = ' ';
                cell.bg = Some(bg_color);
                cell.wide_continuation = false;
            }
        }
    }
}

fn render_text(
    text: &str,
    element: &Element,
    rect: Rect,
    buf: &mut Buffer,
    clip: Option<Rect>,
    animation: &AnimationState,
    oklch_cache: &mut OklchCache,
) {
    // Get foreground color (potentially interpolated)
    let foreground = get_interpolated_color(
        animation,
        &element.id,
        TransitionProperty::Foreground,
        element.style.foreground.as_ref(),
    );
    let fg = foreground
        .as_ref()
        .map(|c| oklch_cache.get(c))
        .unwrap_or(Oklch::new(1.0, 0.0, 0.0)); // white

    // Get background color (potentially interpolated)
    let background = get_interpolated_color(
        animation,
        &element.id,
        TransitionProperty::Background,
        element.style.background.as_ref(),
    );
    let explicit_bg = background.as_ref().map(|c| oklch_cache.get(c));

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

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let max_width = inner.width as usize;

    // Get lines based on wrap mode
    let lines: Vec<String> = match element.text_wrap {
        TextWrap::NoWrap => {
            // Split on newlines, but don't wrap within lines
            text.lines().map(|s| s.to_string()).collect()
        }
        TextWrap::WordWrap => wrap_words(text, max_width),
        TextWrap::CharWrap => wrap_chars(text, max_width),
        TextWrap::Truncate => {
            // Single line with truncation
            let first_line = text.lines().next().unwrap_or("");
            vec![truncate_to_width(first_line, max_width)]
        }
    };

    // Render each line
    for (line_idx, line) in lines.iter().enumerate() {
        let y = inner.y + line_idx as u16;

        // Clip if beyond height
        if y >= inner.bottom() {
            break;
        }

        // Skip if clipped vertically
        if let Some(c) = clip {
            if y < c.y || y >= c.bottom() {
                continue;
            }
        }

        // Calculate alignment offset (skip width calculation for left-align)
        let x_offset = if element.text_align == crate::types::TextAlign::Left {
            0
        } else {
            let line_width = display_width(line);
            align_offset(line_width, max_width, element.text_align) as u16
        };
        let mut x = inner.x + x_offset;

        // Render characters
        for ch in line.chars() {
            let ch_w = char_width(ch);

            if ch_w == 0 {
                // Zero-width char (combining mark, etc.) - attach to previous cell
                continue;
            }

            // Check if we have room for the full character width
            if x + ch_w as u16 > inner.right() {
                break;
            }

            // Skip if clipped horizontally
            if let Some(c) = clip {
                if x < c.x || x >= c.right() {
                    x += ch_w as u16;
                    continue;
                }
            }

            // Preserve existing background if no explicit background set
            let bg = explicit_bg.or_else(|| buf.get(x, y).and_then(|c| c.bg));

            buf.set(
                x,
                y,
                Cell::new(ch)
                    .with_fg(fg)
                    .with_bg(bg)
                    .with_style(element.style.text_style),
            );

            // For wide chars (CJK), fill the next cell with a continuation marker
            if ch_w == 2 && x + 1 < inner.right() {
                // Only render continuation if not clipped
                let cont_x = x + 1;
                if clip.is_none_or(|c| cont_x >= c.x && cont_x < c.right()) {
                    let mut continuation = Cell::new(' ')
                        .with_fg(fg)
                        .with_bg(bg)
                        .with_style(element.style.text_style);
                    continuation.wide_continuation = true;
                    buf.set(cont_x, y, continuation);
                }
            }

            x += ch_w as u16;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn render_text_input(
    value: &str,
    cursor: usize,
    selection: Option<(usize, usize)>,
    placeholder: Option<&str>,
    focused: bool,
    element: &Element,
    rect: Rect,
    buf: &mut Buffer,
    clip: Option<Rect>,
    animation: &AnimationState,
    oklch_cache: &mut OklchCache,
) {
    // Get foreground color (potentially interpolated)
    let foreground = get_interpolated_color(
        animation,
        &element.id,
        TransitionProperty::Foreground,
        element.style.foreground.as_ref(),
    );
    let fg = foreground
        .as_ref()
        .map(|c| oklch_cache.get(c))
        .unwrap_or(Oklch::new(1.0, 0.0, 0.0)); // white

    // Get background color (potentially interpolated)
    let background = get_interpolated_color(
        animation,
        &element.id,
        TransitionProperty::Background,
        element.style.background.as_ref(),
    );
    let bg = background.as_ref().map(|c| oklch_cache.get(c));

    // Cursor style: bright background, dark foreground
    let cursor_fg = Oklch::new(0.15, 0.0, 0.0);
    let cursor_bg = Some(Oklch::new(0.85, 0.0, 0.0));

    // Selection style: medium contrast
    let selection_fg = Oklch::new(0.95, 0.0, 0.0);
    let selection_bg = Some(Oklch::new(0.4, 0.1, 220.0));

    // Placeholder style: dimmed
    let placeholder_fg = Oklch::new(0.5, 0.0, 0.0);

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

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    // Determine what text to display
    // Show placeholder only when empty AND not focused
    let is_placeholder = value.is_empty() && !focused;
    let display_text = if is_placeholder {
        placeholder.unwrap_or("")
    } else {
        value
    };

    let chars: Vec<char> = display_text.chars().collect();
    let y = inner.y;

    // Check vertical clip
    if let Some(c) = clip {
        if y < c.y || y >= c.bottom() {
            return;
        }
    }

    // Calculate horizontal scroll offset to keep cursor visible with margin
    // Always keep cursor at least 5 chars from the right edge for smooth scrolling
    let visible_width = inner.width as usize;
    let cursor_margin = visible_width.min(5);

    let scroll_offset = if focused && !is_placeholder && visible_width > cursor_margin {
        // Calculate the display width up to the cursor (including cursor position)
        let width_to_cursor: usize = chars
            .iter()
            .take(cursor)
            .map(|&c| char_width(c))
            .sum::<usize>()
            + 1; // +1 for cursor itself

        // Scroll when cursor would be within margin of right edge
        let usable_width = visible_width - cursor_margin;
        if width_to_cursor > usable_width {
            let target_scroll_width = width_to_cursor - usable_width;

            // Convert target scroll width to character offset
            let mut offset = 0;
            let mut skipped_width = 0;
            for &ch in &chars {
                if skipped_width >= target_scroll_width {
                    break;
                }
                skipped_width += char_width(ch);
                offset += 1;
            }
            offset
        } else {
            0
        }
    } else {
        0
    };

    let mut x = inner.x;

    for (i, &ch) in chars.iter().enumerate().skip(scroll_offset) {
        if x >= inner.right() {
            break;
        }

        // Check horizontal clip
        if let Some(c) = clip {
            if x < c.x {
                x += char_width(ch) as u16;
                continue;
            }
            if x >= c.right() {
                break;
            }
        }

        // Determine style for this character
        let (char_fg, char_bg) = if is_placeholder {
            (placeholder_fg, bg)
        } else if focused {
            let in_selection = selection
                .map(|(start, end)| i >= start && i < end)
                .unwrap_or(false);
            let is_cursor = i == cursor;

            if is_cursor {
                (cursor_fg, cursor_bg)
            } else if in_selection {
                (selection_fg, selection_bg)
            } else {
                (fg, bg)
            }
        } else {
            (fg, bg)
        };

        buf.set(
            x,
            y,
            Cell::new(ch)
                .with_fg(char_fg)
                .with_bg(char_bg)
                .with_style(element.style.text_style),
        );

        let ch_w = char_width(ch);
        if ch_w == 2 && x + 1 < inner.right() {
            let cont_x = x + 1;
            if clip.is_none_or(|c| cont_x >= c.x && cont_x < c.right()) {
                let mut continuation = Cell::new(' ')
                    .with_fg(char_fg)
                    .with_bg(char_bg)
                    .with_style(element.style.text_style);
                continuation.wide_continuation = true;
                buf.set(cont_x, y, continuation);
            }
        }

        x += ch_w as u16;
    }

    // If cursor is at end and focused, render cursor block
    // Account for scroll offset when determining if cursor is visible
    if focused && cursor >= chars.len() {
        let width_to_cursor: usize = chars
            .iter()
            .skip(scroll_offset)
            .map(|&c| char_width(c))
            .sum();
        let cursor_x = inner.x + width_to_cursor as u16;
        if cursor_x < inner.right() {
            if clip.is_none_or(|c| cursor_x >= c.x && cursor_x < c.right()) {
                buf.set(
                    cursor_x,
                    y,
                    Cell::new(' ').with_fg(cursor_fg).with_bg(cursor_bg),
                );
            }
        }
    }
}

fn render_border(
    element: &Element,
    rect: Rect,
    buf: &mut Buffer,
    clip: Option<Rect>,
    animation: &AnimationState,
    oklch_cache: &mut OklchCache,
) {
    use crate::types::Border;

    let (tl, tr, bl, br, h, v) = match element.style.border {
        Border::None => return,
        Border::Single => ('┌', '┐', '└', '┘', '─', '│'),
        Border::Double => ('╔', '╗', '╚', '╝', '═', '║'),
        Border::Rounded => ('╭', '╮', '╰', '╯', '─', '│'),
        Border::Thick => ('┏', '┓', '┗', '┛', '━', '┃'),
    };

    // Get foreground color (potentially interpolated)
    let foreground = get_interpolated_color(
        animation,
        &element.id,
        TransitionProperty::Foreground,
        element.style.foreground.as_ref(),
    );
    let fg = foreground
        .as_ref()
        .map(|c| oklch_cache.get(c))
        .unwrap_or(Oklch::new(1.0, 0.0, 0.0)); // white

    if rect.width < 2 || rect.height < 2 {
        return;
    }

    // Helper to check if a point is within clip bounds
    let is_visible = |x: u16, y: u16| -> bool {
        clip.is_none_or(|c| x >= c.x && x < c.right() && y >= c.y && y < c.bottom())
    };

    // Corners
    if is_visible(rect.x, rect.y) {
        set_char(buf, rect.x, rect.y, tl, fg);
    }
    if is_visible(rect.right() - 1, rect.y) {
        set_char(buf, rect.right() - 1, rect.y, tr, fg);
    }
    if is_visible(rect.x, rect.bottom() - 1) {
        set_char(buf, rect.x, rect.bottom() - 1, bl, fg);
    }
    if is_visible(rect.right() - 1, rect.bottom() - 1) {
        set_char(buf, rect.right() - 1, rect.bottom() - 1, br, fg);
    }

    // Horizontal lines
    for x in (rect.x + 1)..(rect.right() - 1) {
        if is_visible(x, rect.y) {
            set_char(buf, x, rect.y, h, fg);
        }
        if is_visible(x, rect.bottom() - 1) {
            set_char(buf, x, rect.bottom() - 1, h, fg);
        }
    }

    // Vertical lines
    for y in (rect.y + 1)..(rect.bottom() - 1) {
        if is_visible(rect.x, y) {
            set_char(buf, rect.x, y, v, fg);
        }
        if is_visible(rect.right() - 1, y) {
            set_char(buf, rect.right() - 1, y, v, fg);
        }
    }
}

fn set_char(buf: &mut Buffer, x: u16, y: u16, ch: char, fg: Oklch) {
    if let Some(cell) = buf.get_mut(x, y) {
        cell.char = ch;
        cell.fg = fg;
        // Preserve existing background
    }
}

/// Render scrollbars for elements with Scroll or Auto overflow.
/// For Auto, scrollbars only show if content actually overflows the container.
fn render_scrollbar(
    element: &Element,
    rect: Rect,
    buf: &mut Buffer,
    clip: Option<Rect>,
    content_size: Option<(u16, u16)>,
    viewport_size: Option<(u16, u16)>,
) {
    // Only render scrollbar for Scroll/Auto
    if element.overflow != Overflow::Scroll && element.overflow != Overflow::Auto {
        return;
    }

    let border_size = if element.style.border == crate::types::Border::None {
        0
    } else {
        1
    };

    // Use viewport size from layout if available, otherwise fall back to border-only calculation
    let (inner_width, inner_height) = viewport_size.unwrap_or((
        rect.width.saturating_sub(border_size * 2),
        rect.height.saturating_sub(border_size * 2),
    ));

    let (scroll_x, scroll_y) = element.scroll_offset;
    let (content_width, content_height) = content_size.unwrap_or((inner_width, inner_height));

    log::debug!(
        "render_scrollbar id={} content=({}, {}) viewport=({}, {}) scroll=({}, {})",
        element.id, content_width, content_height, inner_width, inner_height, scroll_x, scroll_y
    );

    // Determine if content overflows
    let overflows_vertical = content_height > inner_height;
    let overflows_horizontal = content_width > inner_width;

    // For Auto, only show scrollbar if content overflows
    // For Scroll, always show the scrollbar
    let show_vertical = element.overflow == Overflow::Scroll || overflows_vertical;
    let show_horizontal = element.overflow == Overflow::Scroll || overflows_horizontal;

    log::debug!(
        "  overflows_v={} overflows_h={} show_v={} show_h={}",
        overflows_vertical, overflows_horizontal, show_vertical, show_horizontal
    );

    // Scrollbar colors in OKLCH (gray tones)
    let track_color = Oklch::from_rgb(crate::types::Rgb::new(60, 60, 60));
    let thumb_color = Oklch::from_rgb(crate::types::Rgb::new(150, 150, 150));

    // Helper to check if a point is within clip bounds
    let is_visible = |x: u16, y: u16| -> bool {
        clip.is_none_or(|c| x >= c.x && x < c.right() && y >= c.y && y < c.bottom())
    };

    // Vertical scrollbar (right edge, inside border)
    if show_vertical && rect.height > 2 + border_size * 2 {
        let x = rect.right() - 1 - border_size;
        let track_start = rect.y + border_size;
        let track_end = rect.bottom() - border_size;
        let track_height = track_end.saturating_sub(track_start);

        if track_height > 0 {
            // Calculate thumb size proportional to visible area
            let thumb_size = if content_height > 0 {
                ((inner_height as u32 * track_height as u32) / content_height as u32)
                    .max(1)
                    .min(track_height as u32) as u16
            } else {
                track_height
            };

            // Calculate thumb position based on scroll offset
            let max_scroll = content_height.saturating_sub(inner_height);
            let scroll_range = track_height.saturating_sub(thumb_size);
            let thumb_pos = if max_scroll > 0 && scroll_range > 0 {
                ((scroll_y as u32 * scroll_range as u32) / max_scroll as u32)
                    .min(scroll_range as u32) as u16
            } else {
                0
            };

            // Draw track
            for y in track_start..track_end {
                if is_visible(x, y) {
                    if let Some(cell) = buf.get_mut(x, y) {
                        let in_thumb = y >= track_start + thumb_pos
                            && y < track_start + thumb_pos + thumb_size;
                        cell.char = if in_thumb { '█' } else { '░' };
                        cell.fg = if in_thumb { thumb_color } else { track_color };
                    }
                }
            }
        }
    }

    // Horizontal scrollbar (bottom edge, inside border)
    if show_horizontal && rect.width > 2 + border_size * 2 {
        let y = rect.bottom() - 1 - border_size;
        let track_start = rect.x + border_size;
        let track_end = rect.right() - border_size;
        // Reduce width if vertical scrollbar is shown
        let track_end = if show_vertical {
            track_end.saturating_sub(1)
        } else {
            track_end
        };
        let track_width = track_end.saturating_sub(track_start);

        if track_width > 0 {
            // Calculate thumb size proportional to visible area
            let thumb_size = if content_width > 0 {
                ((inner_width as u32 * track_width as u32) / content_width as u32)
                    .max(1)
                    .min(track_width as u32) as u16
            } else {
                track_width
            };

            // Calculate thumb position based on scroll offset
            let max_scroll = content_width.saturating_sub(inner_width);
            let scroll_range = track_width.saturating_sub(thumb_size);
            let thumb_pos = if max_scroll > 0 && scroll_range > 0 {
                ((scroll_x as u32 * scroll_range as u32) / max_scroll as u32)
                    .min(scroll_range as u32) as u16
            } else {
                0
            };

            // Draw track
            for x in track_start..track_end {
                if is_visible(x, y) {
                    if let Some(cell) = buf.get_mut(x, y) {
                        let in_thumb = x >= track_start + thumb_pos
                            && x < track_start + thumb_pos + thumb_size;
                        cell.char = if in_thumb { '█' } else { '░' };
                        cell.fg = if in_thumb { thumb_color } else { track_color };
                    }
                }
            }
        }
    }
}

/// Apply backdrop effect to the entire buffer.
/// This is used for modal-like effects where the background is dimmed.
fn apply_backdrop(buf: &mut Buffer, backdrop: &Backdrop) {
    match backdrop {
        Backdrop::None => {}
        Backdrop::Dim(amount) => {
            for y in 0..buf.height() {
                for x in 0..buf.width() {
                    if let Some(cell) = buf.get_mut(x, y) {
                        cell.fg = cell.fg.darken(*amount);
                        cell.bg = cell.bg.map(|bg| bg.darken(*amount));
                    }
                }
            }
        }
        Backdrop::Desaturate(amount) => {
            for y in 0..buf.height() {
                for x in 0..buf.width() {
                    if let Some(cell) = buf.get_mut(x, y) {
                        cell.fg = cell.fg.desaturate(*amount);
                        cell.bg = cell.bg.map(|bg| bg.desaturate(*amount));
                    }
                }
            }
        }
    }
}
