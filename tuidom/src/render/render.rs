use crate::buffer::{Buffer, Cell};
use crate::element::{Content, Element};
use crate::layout::{LayoutResult, Rect};
use crate::types::Rgb;

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
                cell.bg = bg;
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

    let mut x = inner.x;
    let y = inner.y;

    for ch in text.chars() {
        if x >= inner.right() {
            break;
        }
        if y >= inner.bottom() {
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
        x += 1;
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
