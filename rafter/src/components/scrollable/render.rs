//! Scrollable component rendering.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style as RatatuiStyle;

use super::state::{ScrollDirection, ScrollbarConfig, ScrollbarVisibility};
use crate::theme::Theme;

/// Render state for a scrollable, computed during rendering.
pub struct ScrollableRenderState {
    /// Area for the content (excluding scrollbars).
    pub content_area: Rect,
    /// Whether to show vertical scrollbar.
    pub show_vertical: bool,
    /// Whether to show horizontal scrollbar.
    pub show_horizontal: bool,
}

/// Calculate the layout and determine scrollbar visibility.
pub fn calculate_scrollable_layout(
    area: Rect,
    content_size: (u16, u16),
    direction: ScrollDirection,
    config: &ScrollbarConfig,
) -> ScrollableRenderState {
    let (content_width, content_height) = content_size;

    // Determine if scrollbars are needed based on visibility settings
    let needs_vertical = match direction {
        ScrollDirection::Horizontal => false,
        _ => content_height > area.height,
    };

    let needs_horizontal = match direction {
        ScrollDirection::Vertical => false,
        _ => content_width > area.width,
    };

    let show_vertical = match config.vertical {
        ScrollbarVisibility::Always => matches!(direction, ScrollDirection::Vertical | ScrollDirection::Both),
        ScrollbarVisibility::Never => false,
        ScrollbarVisibility::Auto => needs_vertical,
    };

    let show_horizontal = match config.horizontal {
        ScrollbarVisibility::Always => matches!(direction, ScrollDirection::Horizontal | ScrollDirection::Both),
        ScrollbarVisibility::Never => false,
        ScrollbarVisibility::Auto => needs_horizontal,
    };

    // Calculate content area (subtract space for scrollbars)
    let scrollbar_width = if show_vertical { 1 } else { 0 };
    let scrollbar_height = if show_horizontal { 1 } else { 0 };

    let content_area = Rect {
        x: area.x,
        y: area.y,
        width: area.width.saturating_sub(scrollbar_width),
        height: area.height.saturating_sub(scrollbar_height),
    };

    ScrollableRenderState {
        content_area,
        show_vertical,
        show_horizontal,
    }
}

/// Render the vertical scrollbar.
pub fn render_vertical_scrollbar(
    buf: &mut Buffer,
    area: Rect,
    offset_y: u16,
    content_height: u16,
    viewport_height: u16,
    config: &ScrollbarConfig,
    theme: &dyn Theme,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    // Scrollbar is rendered in the rightmost column
    let scrollbar_x = area.x + area.width - 1;
    let scrollbar_height = area.height;

    // Resolve colors
    let track_color = config
        .track_color
        .as_ref()
        .and_then(|c| crate::theme::resolve_style_color(c, theme))
        .unwrap_or_else(|| crate::color::Color::rgb(60, 60, 70));

    let handle_color = config
        .handle_color
        .as_ref()
        .and_then(|c| crate::theme::resolve_style_color(c, theme))
        .unwrap_or_else(|| crate::color::Color::rgb(120, 120, 140));

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
        let cell = buf.cell_mut((scrollbar_x, area.y + y)).unwrap();
        cell.set_char(' ');

        if y >= handle_pos && y < handle_pos + handle_height {
            cell.set_style(handle_style);
        } else {
            cell.set_style(track_style);
        }
    }
}

/// Render the horizontal scrollbar.
pub fn render_horizontal_scrollbar(
    buf: &mut Buffer,
    area: Rect,
    offset_x: u16,
    content_width: u16,
    viewport_width: u16,
    config: &ScrollbarConfig,
    theme: &dyn Theme,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    // Scrollbar is rendered in the bottom row
    let scrollbar_y = area.y + area.height - 1;
    let scrollbar_width = area.width;

    // Resolve colors
    let track_color = config
        .track_color
        .as_ref()
        .and_then(|c| crate::theme::resolve_style_color(c, theme))
        .unwrap_or_else(|| crate::color::Color::rgb(60, 60, 70));

    let handle_color = config
        .handle_color
        .as_ref()
        .and_then(|c| crate::theme::resolve_style_color(c, theme))
        .unwrap_or_else(|| crate::color::Color::rgb(120, 120, 140));

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
        let cell = buf.cell_mut((area.x + x, scrollbar_y)).unwrap();
        cell.set_char(' ');

        if x >= handle_pos && x < handle_pos + handle_width {
            cell.set_style(handle_style);
        } else {
            cell.set_style(track_style);
        }
    }
}
