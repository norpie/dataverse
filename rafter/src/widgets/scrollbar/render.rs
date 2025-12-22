//! Scrollbar rendering functions.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style as RatatuiStyle;

use super::types::{ScrollbarConfig, ScrollbarGeometry};
use crate::styling::theme::Theme;

/// Render the vertical scrollbar and return its geometry.
///
/// The scrollbar is rendered in the rightmost column of the given area.
pub fn render_vertical_scrollbar(
    buf: &mut Buffer,
    area: Rect,
    offset_y: u16,
    content_height: u16,
    viewport_height: u16,
    config: &ScrollbarConfig,
    theme: &dyn Theme,
) -> Option<ScrollbarGeometry> {
    if area.width == 0 || area.height == 0 {
        return None;
    }

    // Scrollbar is rendered in the rightmost column
    let scrollbar_x = area.x + area.width - 1;
    let scrollbar_height = area.height;

    // Resolve colors
    let track_color = config
        .track_color
        .as_ref()
        .and_then(|c| crate::styling::theme::resolve_style_color(c, theme))
        .unwrap_or_else(|| crate::styling::color::Color::rgb(60, 60, 70));

    let handle_color = config
        .handle_color
        .as_ref()
        .and_then(|c| crate::styling::theme::resolve_style_color(c, theme))
        .unwrap_or_else(|| crate::styling::color::Color::rgb(120, 120, 140));

    let track_style = RatatuiStyle::default().bg(track_color.to_ratatui());
    let handle_style = RatatuiStyle::default().bg(handle_color.to_ratatui());

    // Calculate handle size and position
    let visible_ratio = viewport_height as f32 / content_height.max(1) as f32;
    let handle_height = ((scrollbar_height as f32 * visible_ratio).ceil() as u16).max(1);

    let scrollable_range = content_height.saturating_sub(viewport_height);
    let handle_range = scrollbar_height.saturating_sub(handle_height);

    let handle_pos = if scrollable_range > 0 {
        ((offset_y as f32 / scrollable_range as f32) * handle_range as f32).round() as u16
    } else {
        0
    };

    // Render track and handle
    for y in 0..scrollbar_height {
        if let Some(cell) = buf.cell_mut((scrollbar_x, area.y + y)) {
            cell.set_char(' ');

            if y >= handle_pos && y < handle_pos + handle_height {
                cell.set_style(handle_style);
            } else {
                cell.set_style(track_style);
            }
        }
    }

    Some(ScrollbarGeometry {
        x: scrollbar_x,
        y: area.y,
        width: 1,
        height: scrollbar_height,
        handle_pos,
        handle_size: handle_height,
    })
}

/// Render the horizontal scrollbar and return its geometry.
///
/// The scrollbar is rendered in the bottom row of the given area.
pub fn render_horizontal_scrollbar(
    buf: &mut Buffer,
    area: Rect,
    offset_x: u16,
    content_width: u16,
    viewport_width: u16,
    config: &ScrollbarConfig,
    theme: &dyn Theme,
) -> Option<ScrollbarGeometry> {
    if area.width == 0 || area.height == 0 {
        return None;
    }

    // Scrollbar is rendered in the bottom row
    let scrollbar_y = area.y + area.height - 1;
    let scrollbar_width = area.width;

    // Resolve colors
    let track_color = config
        .track_color
        .as_ref()
        .and_then(|c| crate::styling::theme::resolve_style_color(c, theme))
        .unwrap_or_else(|| crate::styling::color::Color::rgb(60, 60, 70));

    let handle_color = config
        .handle_color
        .as_ref()
        .and_then(|c| crate::styling::theme::resolve_style_color(c, theme))
        .unwrap_or_else(|| crate::styling::color::Color::rgb(120, 120, 140));

    let track_style = RatatuiStyle::default().bg(track_color.to_ratatui());
    let handle_style = RatatuiStyle::default().bg(handle_color.to_ratatui());

    // Calculate handle size and position
    let visible_ratio = viewport_width as f32 / content_width.max(1) as f32;
    let handle_width = ((scrollbar_width as f32 * visible_ratio).ceil() as u16).max(1);

    let scrollable_range = content_width.saturating_sub(viewport_width);
    let handle_range = scrollbar_width.saturating_sub(handle_width);

    let handle_pos = if scrollable_range > 0 {
        ((offset_x as f32 / scrollable_range as f32) * handle_range as f32).round() as u16
    } else {
        0
    };

    // Render track and handle
    for x in 0..scrollbar_width {
        if let Some(cell) = buf.cell_mut((area.x + x, scrollbar_y)) {
            cell.set_char(' ');

            if x >= handle_pos && x < handle_pos + handle_width {
                cell.set_style(handle_style);
            } else {
                cell.set_style(track_style);
            }
        }
    }

    Some(ScrollbarGeometry {
        x: area.x,
        y: scrollbar_y,
        width: scrollbar_width,
        height: 1,
        handle_pos,
        handle_size: handle_width,
    })
}
