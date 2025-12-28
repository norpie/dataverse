use crate::buffer::{Buffer, Cell};
use crate::element::{Content, Element};
use crate::layout::{LayoutResult, Rect};
use crate::text::{align_offset, char_width, display_width, truncate_to_width, wrap_chars, wrap_words};
use crate::types::{Rgb, TextWrap};

pub fn render_to_buffer(element: &Element, layout: &LayoutResult, buf: &mut Buffer) {
    render_element(element, layout, buf);
}

fn render_element(element: &Element, layout: &LayoutResult, buf: &mut Buffer) {
    let Some(rect) = layout.get(&element.id) else {
        return;
    };

    // Render background if set
    if let Some(bg) = &element.style.background {
        let rgb = bg.to_rgb();
        fill_rect(buf, *rect, rgb);
    }

    // Render border if set
    render_border(element, *rect, buf);

    // Render content
    match &element.content {
        Content::None => {}
        Content::Text(text) => {
            render_text(text, element, *rect, buf);
        }
        Content::Children(children) => {
            for child in children {
                render_element(child, layout, buf);
            }
        }
        Content::Custom(custom) => {
            custom.render(*rect, buf);
        }
    }
}

fn fill_rect(buf: &mut Buffer, rect: Rect, bg: Rgb) {
    for y in rect.y..rect.bottom().min(buf.height()) {
        for x in rect.x..rect.right().min(buf.width()) {
            if let Some(cell) = buf.get_mut(x, y) {
                cell.char = ' ';
                cell.bg = bg;
                cell.wide_continuation = false;
            }
        }
    }
}

fn render_text(text: &str, element: &Element, rect: Rect, buf: &mut Buffer) {
    let fg = element
        .style
        .foreground
        .as_ref()
        .map(|c| c.to_rgb())
        .unwrap_or(Rgb::new(255, 255, 255));

    let explicit_bg = element.style.background.as_ref().map(|c| c.to_rgb());

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

        // Calculate alignment offset
        let line_width = display_width(line);
        let x_offset = align_offset(line_width, max_width, element.text_align) as u16;
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

            // Preserve existing background if no explicit background set
            let bg = explicit_bg.unwrap_or_else(|| {
                buf.get(x, y).map(|c| c.bg).unwrap_or(Rgb::new(0, 0, 0))
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
                let mut continuation = Cell::new(' ')
                    .with_fg(fg)
                    .with_bg(bg)
                    .with_style(element.style.text_style);
                continuation.wide_continuation = true;
                buf.set(x + 1, y, continuation);
            }

            x += ch_w as u16;
        }
    }
}

fn render_border(element: &Element, rect: Rect, buf: &mut Buffer) {
    use crate::types::Border;

    let (tl, tr, bl, br, h, v) = match element.style.border {
        Border::None => return,
        Border::Single => ('┌', '┐', '└', '┘', '─', '│'),
        Border::Double => ('╔', '╗', '╚', '╝', '═', '║'),
        Border::Rounded => ('╭', '╮', '╰', '╯', '─', '│'),
        Border::Thick => ('┏', '┓', '┗', '┛', '━', '┃'),
    };

    let fg = element
        .style
        .foreground
        .as_ref()
        .map(|c| c.to_rgb())
        .unwrap_or(Rgb::new(255, 255, 255));

    if rect.width < 2 || rect.height < 2 {
        return;
    }

    // Corners
    set_char(buf, rect.x, rect.y, tl, fg);
    set_char(buf, rect.right() - 1, rect.y, tr, fg);
    set_char(buf, rect.x, rect.bottom() - 1, bl, fg);
    set_char(buf, rect.right() - 1, rect.bottom() - 1, br, fg);

    // Horizontal lines
    for x in (rect.x + 1)..(rect.right() - 1) {
        set_char(buf, x, rect.y, h, fg);
        set_char(buf, x, rect.bottom() - 1, h, fg);
    }

    // Vertical lines
    for y in (rect.y + 1)..(rect.bottom() - 1) {
        set_char(buf, rect.x, y, v, fg);
        set_char(buf, rect.right() - 1, y, v, fg);
    }
}

fn set_char(buf: &mut Buffer, x: u16, y: u16, ch: char, fg: Rgb) {
    if let Some(cell) = buf.get_mut(x, y) {
        cell.char = ch;
        cell.fg = fg;
        // Preserve existing background
    }
}
