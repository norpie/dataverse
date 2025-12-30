use std::collections::HashMap;
use std::time::Instant;

use crate::animation::{AnimationState, PropertyValue, TransitionProperty};
use crate::buffer::{Buffer, Cell};
use crate::element::{Content, Element};
use crate::layout::{LayoutResult, Rect};
use crate::text::{
    align_offset, char_width, display_width, truncate_to_width, wrap_chars, wrap_words,
};
use crate::types::{Backdrop, Color, ColorKey, Oklch, Overflow, TextWrap};

/// Cache for Color → Oklch conversions during render.
/// Avoids repeated palette conversions for the same colors within a frame.
struct OklchCache {
    cache: HashMap<ColorKey, Oklch>,
}

impl OklchCache {
    fn new() -> Self {
        Self {
            cache: HashMap::with_capacity(32),
        }
    }

    fn get(&mut self, color: &Color) -> Oklch {
        if let Some(key) = color.cache_key() {
            *self.cache.entry(key).or_insert_with(|| color.to_oklch())
        } else {
            // Derived colors: compute without caching
            color.to_oklch()
        }
    }
}

/// A render item contains an element with its z_index, tree order, and clip rect.
struct RenderItem<'a> {
    element: &'a Element,
    z_index: i16,
    tree_order: usize,
    clip: Option<Rect>,
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
) {
    let t0 = Instant::now();

    // Collect all elements with their effective z_index, tree order, and clip rects
    let mut render_list: Vec<RenderItem> = Vec::new();
    collect_elements(element, layout, &mut render_list, 0, element.z_index, None);
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

    // Create color cache for this frame
    let mut oklch_cache = OklchCache::new();

    // Render in sorted order
    for item in render_list {
        render_single_element_timed(
            item.element,
            layout,
            buf,
            item.clip,
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
) -> usize {
    let mut order = tree_order;
    // Effective z_index: use element's z_index if explicitly higher, otherwise inherit parent's
    let effective_z = element.z_index.max(parent_z_index);

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
    });
    order += 1;

    if let Content::Children(children) = &element.content {
        for child in children {
            order = collect_elements(child, layout, list, order, effective_z, child_clip);
        }
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

    // Adjust rect for interpolated position (for Relative/Absolute elements)
    let rect = adjust_rect_for_position(element, *layout_rect, animation);

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
        fill_rect(buf, visible_rect, oklch);
    }
    let t2 = Instant::now();
    stats.background_us += t2.duration_since(t1).as_secs_f64() * 1_000_000.0;

    // Render border if set (needs full rect for corners, but clip to visible)
    render_border(element, rect, buf, clip, animation, oklch_cache);
    let t3 = Instant::now();
    stats.border_us += t3.duration_since(t2).as_secs_f64() * 1_000_000.0;

    // Render content (text or custom only, children handled separately)
    match &element.content {
        Content::None | Content::Children(_) => {}
        Content::Text(text) => {
            render_text(text, element, rect, buf, clip, animation, oklch_cache);
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

/// Get interpolated i16 value if there's an active transition, otherwise return the current value.
fn get_interpolated_i16(
    animation: &AnimationState,
    element_id: &str,
    property: TransitionProperty,
    current: Option<i16>,
) -> Option<i16> {
    // Check for active transition
    if let Some(PropertyValue::I16(val)) = animation.get_interpolated(element_id, property) {
        return Some(val);
    }
    // No transition, use current value
    current
}

/// Adjust rect based on interpolated position offsets for Relative/Absolute positioned elements.
fn adjust_rect_for_position(element: &Element, rect: Rect, animation: &AnimationState) -> Rect {
    use crate::types::Position;

    // Only adjust for Relative or Absolute positioned elements
    if element.position != Position::Relative && element.position != Position::Absolute {
        return rect;
    }

    // Get interpolated offsets (or current if no transition)
    let left = get_interpolated_i16(
        animation,
        &element.id,
        TransitionProperty::Left,
        element.left,
    );
    let top = get_interpolated_i16(animation, &element.id, TransitionProperty::Top, element.top);
    let right = get_interpolated_i16(
        animation,
        &element.id,
        TransitionProperty::Right,
        element.right,
    );
    let bottom = get_interpolated_i16(
        animation,
        &element.id,
        TransitionProperty::Bottom,
        element.bottom,
    );

    // Calculate offset from current element values vs interpolated
    let dx = left.unwrap_or(0) - element.left.unwrap_or(0);
    let dy = top.unwrap_or(0) - element.top.unwrap_or(0);

    // For right/bottom, the offset is inverted
    let dx = dx - (right.unwrap_or(0) - element.right.unwrap_or(0));
    let dy = dy - (bottom.unwrap_or(0) - element.bottom.unwrap_or(0));

    // Apply offset to rect
    Rect::new(
        (rect.x as i32 + dx as i32).max(0) as u16,
        (rect.y as i32 + dy as i32).max(0) as u16,
        rect.width,
        rect.height,
    )
}

fn fill_rect(buf: &mut Buffer, rect: Rect, bg: Oklch) {
    for y in rect.y..rect.bottom().min(buf.height()) {
        for x in rect.x..rect.right().min(buf.width()) {
            if let Some(cell) = buf.get_mut(x, y) {
                // Skip if cell already has correct state
                if cell.bg == bg && cell.char == ' ' && !cell.wide_continuation {
                    continue;
                }
                cell.char = ' ';
                cell.bg = bg;
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
            let bg = explicit_bg.unwrap_or_else(|| {
                buf.get(x, y)
                    .map(|c| c.bg)
                    .unwrap_or(Oklch::new(0.0, 0.0, 0.0))
            });

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
                if clip.map_or(true, |c| cont_x >= c.x && cont_x < c.right()) {
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
        clip.map_or(true, |c| {
            x >= c.x && x < c.right() && y >= c.y && y < c.bottom()
        })
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

    // Determine if content overflows
    let overflows_vertical = content_height > inner_height;
    let overflows_horizontal = content_width > inner_width;

    // For Auto, only show scrollbar if content overflows
    // For Scroll, always show the scrollbar
    let show_vertical = element.overflow == Overflow::Scroll || overflows_vertical;
    let show_horizontal = element.overflow == Overflow::Scroll || overflows_horizontal;

    // Scrollbar colors in OKLCH (gray tones)
    let track_color = Oklch::from_rgb(crate::types::Rgb::new(60, 60, 60));
    let thumb_color = Oklch::from_rgb(crate::types::Rgb::new(150, 150, 150));

    // Helper to check if a point is within clip bounds
    let is_visible = |x: u16, y: u16| -> bool {
        clip.map_or(true, |c| {
            x >= c.x && x < c.right() && y >= c.y && y < c.bottom()
        })
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
                        cell.bg = cell.bg.darken(*amount);
                    }
                }
            }
        }
        Backdrop::Desaturate(amount) => {
            for y in 0..buf.height() {
                for x in 0..buf.width() {
                    if let Some(cell) = buf.get_mut(x, y) {
                        cell.fg = cell.fg.desaturate(*amount);
                        cell.bg = cell.bg.desaturate(*amount);
                    }
                }
            }
        }
    }
}
