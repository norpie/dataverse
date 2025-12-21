//! Backdrop dimming and background fill utilities.

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::style::{Color, Style as RatatuiStyle};
use ratatui::widgets::Block;

/// Fill the entire buffer with a background color.
///
/// This should be called before rendering the page to ensure
/// the entire terminal has a consistent background.
pub fn fill_background(frame: &mut Frame, color: Color) {
    let area = frame.area();
    let block = Block::default().style(RatatuiStyle::default().bg(color));
    frame.render_widget(block, area);
}

/// Dim the backdrop buffer by reducing brightness.
///
/// This reduces the brightness of all colors in the buffer by the given amount.
/// An amount of 0.5 will reduce brightness by half.
/// Uses fast integer math instead of color space conversions.
pub fn dim_backdrop(buffer: &mut Buffer, amount: f32) {
    // Pre-calculate the multiplier as an integer for speed (0-256 range)
    let mult = ((1.0 - amount) * 256.0) as u16;

    for cell in buffer.content.iter_mut() {
        cell.bg = dim_color_fast(cell.bg, mult);
        cell.fg = dim_color_fast(cell.fg, mult);
    }
}

/// Fast color dimming using integer multiplication.
/// `mult` is in 0-256 range where 256 = no change, 0 = black.
#[inline]
fn dim_color_fast(color: Color, mult: u16) -> Color {
    match color {
        Color::Rgb(r, g, b) => Color::Rgb(
            ((r as u16 * mult) >> 8) as u8,
            ((g as u16 * mult) >> 8) as u8,
            ((b as u16 * mult) >> 8) as u8,
        ),
        // For basic ANSI colors, convert to RGB and dim
        Color::Black => Color::Rgb(0, 0, 0),
        Color::Red => dim_rgb_fast(205, 49, 49, mult),
        Color::Green => dim_rgb_fast(13, 188, 121, mult),
        Color::Yellow => dim_rgb_fast(229, 229, 16, mult),
        Color::Blue => dim_rgb_fast(36, 114, 200, mult),
        Color::Magenta => dim_rgb_fast(188, 63, 188, mult),
        Color::Cyan => dim_rgb_fast(17, 168, 205, mult),
        Color::Gray => dim_rgb_fast(128, 128, 128, mult),
        Color::DarkGray => dim_rgb_fast(102, 102, 102, mult),
        Color::LightRed => dim_rgb_fast(241, 76, 76, mult),
        Color::LightGreen => dim_rgb_fast(35, 209, 139, mult),
        Color::LightYellow => dim_rgb_fast(245, 245, 67, mult),
        Color::LightBlue => dim_rgb_fast(59, 142, 234, mult),
        Color::LightMagenta => dim_rgb_fast(214, 112, 214, mult),
        Color::LightCyan => dim_rgb_fast(41, 184, 219, mult),
        Color::White => dim_rgb_fast(229, 229, 229, mult),
        // For indexed colors, convert and dim
        Color::Indexed(idx) => dim_indexed_fast(idx, mult),
        // Reset - dim as light gray
        Color::Reset => dim_rgb_fast(200, 200, 200, mult),
    }
}

/// Helper to dim RGB values inline.
#[inline]
fn dim_rgb_fast(r: u8, g: u8, b: u8, mult: u16) -> Color {
    Color::Rgb(
        ((r as u16 * mult) >> 8) as u8,
        ((g as u16 * mult) >> 8) as u8,
        ((b as u16 * mult) >> 8) as u8,
    )
}

/// Dim an indexed color.
#[inline]
fn dim_indexed_fast(idx: u8, mult: u16) -> Color {
    let (r, g, b) = indexed_to_rgb_tuple(idx);
    dim_rgb_fast(r, g, b, mult)
}

/// Convert an ANSI 256 indexed color to RGB tuple.
#[inline]
fn indexed_to_rgb_tuple(idx: u8) -> (u8, u8, u8) {
    match idx {
        // Standard colors (0-15)
        0 => (0, 0, 0),
        1 => (205, 49, 49),
        2 => (13, 188, 121),
        3 => (229, 229, 16),
        4 => (36, 114, 200),
        5 => (188, 63, 188),
        6 => (17, 168, 205),
        7 => (229, 229, 229),
        8 => (102, 102, 102),
        9 => (241, 76, 76),
        10 => (35, 209, 139),
        11 => (245, 245, 67),
        12 => (59, 142, 234),
        13 => (214, 112, 214),
        14 => (41, 184, 219),
        15 => (255, 255, 255),
        // 216 color cube (16-231): 6x6x6 RGB
        16..=231 => {
            let i = idx - 16;
            let r = (i / 36) % 6;
            let g = (i / 6) % 6;
            let b = i % 6;
            let to_255 = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
            (to_255(r), to_255(g), to_255(b))
        }
        // Grayscale (232-255)
        232..=255 => {
            let gray = 8 + (idx - 232) * 10;
            (gray, gray, gray)
        }
    }
}
