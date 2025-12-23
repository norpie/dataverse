//! Table widget rendering.

use std::ops::Range;

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style as RatatuiStyle;

use crate::node::{Layout, Node};
use crate::runtime::hit_test::HitTestMap;
use crate::runtime::render::layout::{apply_border, apply_padding};
use crate::styling::color::Color;
use crate::styling::theme::Theme;
use crate::widgets::scrollbar::{render_horizontal_scrollbar, render_vertical_scrollbar};
use crate::widgets::table::{Alignment, AnyTable, Column};

/// Render a table widget.
#[allow(clippy::too_many_arguments)]
pub fn render(
    frame: &mut Frame,
    id: &str,
    style: RatatuiStyle,
    layout: &Layout,
    widget: &dyn AnyTable,
    area: Rect,
    hit_map: &mut HitTestMap,
    theme: &dyn Theme,
) {
    use ratatui::widgets::Block;

    // Apply border and get inner area
    let (inner_area, block) = apply_border(area, &layout.border, style);
    if let Some(block) = block {
        frame.render_widget(block, area);
    } else if style.bg.is_some() {
        // Fill background if no border but has background
        let bg_block = Block::default().style(style);
        frame.render_widget(bg_block, area);
    }

    // Apply padding
    let padded_area = apply_padding(inner_area, layout.padding);

    if padded_area.width == 0 || padded_area.height == 0 {
        return;
    }

    // Calculate effective table width (clamped to content)
    let table_content_width = widget.total_width();

    // First pass: estimate viewport to determine scrollbar needs
    // Use the smaller of available space and content size
    let estimated_width = padded_area.width.min(table_content_width);
    let estimated_height = padded_area.height;

    widget.set_viewport_width(estimated_width);
    widget.set_viewport_height(estimated_height);

    // Determine scrollbar needs based on content vs viewport
    let needs_v_scrollbar = widget.needs_vertical_scrollbar();
    let needs_h_scrollbar = widget.needs_horizontal_scrollbar();

    // Reserve space for scrollbars
    let v_scrollbar_reserved = if needs_v_scrollbar { 2u16 } else { 0u16 };
    let h_scrollbar_reserved = if needs_h_scrollbar { 1u16 } else { 0u16 };

    // Content area excludes scrollbars
    let content_area = Rect {
        x: padded_area.x,
        y: padded_area.y,
        width: padded_area.width.saturating_sub(v_scrollbar_reserved),
        height: padded_area.height.saturating_sub(h_scrollbar_reserved),
    };

    if content_area.width == 0 || content_area.height == 0 {
        return;
    }

    // Final viewport dimensions (clamped to content width)
    let effective_width = content_area.width.min(table_content_width);
    widget.set_viewport_height(content_area.height);
    widget.set_viewport_width(effective_width);

    // Use effective width for rendering (clamped to content)
    let render_area = Rect {
        x: content_area.x,
        y: content_area.y,
        width: effective_width,
        height: content_area.height,
    };

    // Header is at row 0, data starts at row 1
    let header_height = 1u16;
    let data_area = Rect {
        x: render_area.x,
        y: render_area.y + header_height,
        width: render_area.width,
        height: render_area.height.saturating_sub(header_height),
    };

    let columns = widget.columns();
    let scroll_offset_x = widget.scroll_offset_x();
    let scroll_offset_y = widget.scroll_offset_y();
    let visible_col_range = widget.visible_column_range();
    let visible_row_range = widget.visible_row_range();
    let row_height = widget.row_height();

    // Render header row
    render_header(
        frame,
        &columns,
        widget.sort(),
        Rect {
            x: render_area.x,
            y: render_area.y,
            width: render_area.width,
            height: header_height,
        },
        scroll_offset_x,
        visible_col_range.clone(),
        theme,
    );

    // Calculate column x-positions for rendering
    let mut col_positions: Vec<u16> = Vec::with_capacity(columns.len());
    let mut x_pos = 0u16;
    for col in &columns {
        col_positions.push(x_pos);
        x_pos += col.width;
    }

    // Render visible data rows
    let first_row_y = (visible_row_range.start as u16 * row_height).saturating_sub(scroll_offset_y);

    for (i, row_index) in visible_row_range.clone().enumerate() {
        let row_y = data_area.y + first_row_y + (i as u16 * row_height);

        // Skip if outside viewport
        if row_y >= data_area.y + data_area.height {
            break;
        }

        let row_area = Rect {
            x: data_area.x,
            y: row_y,
            width: data_area.width,
            height: row_height.min(data_area.y + data_area.height - row_y),
        };

        // Render the row with cell-by-cell column widths
        render_row(
            frame,
            widget,
            row_index,
            &columns,
            &col_positions,
            scroll_offset_x,
            visible_col_range.clone(),
            row_area,
            theme,
        );
    }

    // Render vertical scrollbar if needed
    if needs_v_scrollbar {
        let scrollbar_area = Rect {
            x: render_area.x + render_area.width,
            y: render_area.y,
            width: 1,
            height: render_area.height.saturating_sub(h_scrollbar_reserved),
        };

        let config = widget.scrollbar_config();
        let v_geom = render_vertical_scrollbar(
            frame.buffer_mut(),
            scrollbar_area,
            scroll_offset_y,
            widget.total_height(),
            widget.data_viewport_height(),
            &config,
            theme,
        );
        widget.set_vertical_scrollbar(v_geom);
    } else {
        widget.set_vertical_scrollbar(None);
    }

    // Render horizontal scrollbar if needed
    if needs_h_scrollbar {
        let scrollbar_area = Rect {
            x: render_area.x,
            y: render_area.y + render_area.height,
            width: render_area.width + v_scrollbar_reserved,
            height: 1,
        };

        let config = widget.scrollbar_config();
        let h_geom = render_horizontal_scrollbar(
            frame.buffer_mut(),
            scrollbar_area,
            scroll_offset_x,
            widget.total_width(),
            content_area.width,
            &config,
            theme,
        );
        widget.set_horizontal_scrollbar(h_geom);
    } else {
        widget.set_horizontal_scrollbar(None);
    }

    // Register hit box for the content area
    if !id.is_empty() {
        hit_map.register(id.to_string(), padded_area, true);
    }
}

/// Render the table header row.
pub fn render_header(
    frame: &mut Frame,
    columns: &[Column],
    sort: Option<(usize, bool)>,
    area: Rect,
    scroll_offset_x: u16,
    visible_col_range: Range<usize>,
    theme: &dyn Theme,
) {
    use ratatui::style::{Modifier, Style as RStyle};
    use ratatui::text::Span;

    // Get theme colors for header
    let header_bg = theme.resolve("surface").unwrap_or(Color::hex(0x1e1e2e));
    let header_fg = theme.resolve("text").unwrap_or(Color::hex(0xcdd6f4));
    let header_style = RStyle::default()
        .fg(ratatui::style::Color::Rgb(
            header_fg.r(),
            header_fg.g(),
            header_fg.b(),
        ))
        .bg(ratatui::style::Color::Rgb(
            header_bg.r(),
            header_bg.g(),
            header_bg.b(),
        ))
        .add_modifier(Modifier::BOLD);

    // Calculate column x-positions
    let mut col_positions: Vec<u16> = Vec::with_capacity(columns.len());
    let mut x_pos = 0u16;
    for col in columns {
        col_positions.push(x_pos);
        x_pos += col.width;
    }

    // Render visible columns
    for col_idx in visible_col_range {
        if col_idx >= columns.len() {
            break;
        }
        let col = &columns[col_idx];
        let col_start_x = col_positions[col_idx];

        // Calculate position relative to viewport
        let rel_x = col_start_x.saturating_sub(scroll_offset_x);
        if rel_x >= area.width {
            continue; // Column is fully off-screen to the right
        }

        // Calculate how much of the column is visible
        let visible_start = scroll_offset_x.saturating_sub(col_start_x);
        let visible_width = col
            .width
            .saturating_sub(visible_start)
            .min(area.width - rel_x);

        if visible_width == 0 {
            continue;
        }

        // Build header text with sort indicator
        // Indicator placement depends on alignment to avoid shifting text
        let header_text = if let Some((sort_col, ascending)) = sort
            && sort_col == col_idx
        {
            let indicator = if ascending { "▲" } else { "▼" };
            match col.align {
                Alignment::Left => {
                    // Indicator on right
                    format!("{} {}", col.header, indicator)
                }
                Alignment::Right => {
                    // Indicator on left
                    format!("{} {}", indicator, col.header)
                }
                Alignment::Center => {
                    // Check which side has more room
                    let header_len = col.header.chars().count();
                    let col_width = col.width as usize;
                    let left_padding = col_width.saturating_sub(header_len) / 2;
                    let right_padding = col_width
                        .saturating_sub(header_len)
                        .saturating_sub(left_padding);

                    if left_padding >= right_padding {
                        // More room on left, put indicator there
                        format!("{} {}", indicator, col.header)
                    } else {
                        // More room on right
                        format!("{} {}", col.header, indicator)
                    }
                }
            }
        } else {
            col.header.clone()
        };

        // Apply alignment to full column width first
        let aligned_text = match col.align {
            Alignment::Left => {
                format!("{:<width$}", header_text, width = col.width as usize)
            }
            Alignment::Center => {
                format!("{:^width$}", header_text, width = col.width as usize)
            }
            Alignment::Right => {
                format!("{:>width$}", header_text, width = col.width as usize)
            }
        };

        // Handle partial column visibility due to horizontal scroll
        let display_text: String = if visible_start > 0 {
            // Partial column on the left edge
            aligned_text
                .chars()
                .skip(visible_start as usize)
                .take(visible_width as usize)
                .collect()
        } else {
            aligned_text.chars().take(visible_width as usize).collect()
        };

        // Pad to fill the visible width
        let padded = format!("{:<width$}", display_text, width = visible_width as usize);

        let x = area.x + rel_x;
        let span = Span::styled(padded, header_style);
        frame.buffer_mut().set_span(x, area.y, &span, visible_width);
    }
}

/// Render a single table data row with proper column alignment.
#[allow(clippy::too_many_arguments)]
pub fn render_row(
    frame: &mut Frame,
    widget: &dyn AnyTable,
    row_index: usize,
    columns: &[Column],
    col_positions: &[u16],
    scroll_offset_x: u16,
    visible_col_range: Range<usize>,
    area: Rect,
    theme: &dyn Theme,
) {
    use ratatui::style::Style as RStyle;
    use ratatui::text::Span;

    let is_focused = widget.is_focused_at(row_index);
    let is_selected = widget.is_selected_at(row_index);

    // Determine row background based on focus/selection state
    let row_style = if is_focused {
        // Bright purple for cursor
        let bg = Color::hex(0xA277FF);
        let fg = theme.resolve("background").unwrap_or(Color::hex(0x1e1e2e));
        RStyle::default()
            .bg(ratatui::style::Color::Rgb(bg.r(), bg.g(), bg.b()))
            .fg(ratatui::style::Color::Rgb(fg.r(), fg.g(), fg.b()))
    } else if is_selected {
        // Dimmer purple for selected
        let bg = Color::hex(0x6E5494);
        let fg = theme.resolve("background").unwrap_or(Color::hex(0x1e1e2e));
        RStyle::default()
            .bg(ratatui::style::Color::Rgb(bg.r(), bg.g(), bg.b()))
            .fg(ratatui::style::Color::Rgb(fg.r(), fg.g(), fg.b()))
    } else {
        RStyle::default()
    };

    // Fill the entire row with background first (for focused/selected rows)
    if is_focused || is_selected {
        let fill = " ".repeat(area.width as usize);
        let span = Span::styled(fill, row_style);
        frame
            .buffer_mut()
            .set_span(area.x, area.y, &span, area.width);
    }

    // Render each visible column
    for col_idx in visible_col_range {
        if col_idx >= columns.len() || col_idx >= col_positions.len() {
            break;
        }
        let col = &columns[col_idx];
        let col_start_x = col_positions[col_idx];

        // Calculate position relative to viewport
        let rel_x = col_start_x.saturating_sub(scroll_offset_x);
        if rel_x >= area.width {
            continue; // Column is fully off-screen to the right
        }

        // Calculate how much of the column is visible
        let visible_start = scroll_offset_x.saturating_sub(col_start_x);
        let visible_width = col
            .width
            .saturating_sub(visible_start)
            .min(area.width - rel_x);

        if visible_width == 0 {
            continue;
        }

        // Get cell content as text
        let cell_text = if let Some(cell_node) = widget.render_cell(row_index, col_idx) {
            extract_text_from_node(&cell_node)
        } else {
            String::new()
        };

        // Apply alignment and truncate/pad to column width
        let full_cell_text = match col.align {
            Alignment::Left => format!("{:<width$}", cell_text, width = col.width as usize),
            Alignment::Center => format!("{:^width$}", cell_text, width = col.width as usize),
            Alignment::Right => format!("{:>width$}", cell_text, width = col.width as usize),
        };

        // Handle partial column visibility due to horizontal scroll
        let display_text: String = if visible_start > 0 {
            // Partial column on the left edge
            full_cell_text
                .chars()
                .skip(visible_start as usize)
                .take(visible_width as usize)
                .collect()
        } else {
            full_cell_text
                .chars()
                .take(visible_width as usize)
                .collect()
        };

        // Pad to fill the visible width
        let padded = format!("{:<width$}", display_text, width = visible_width as usize);

        let x = area.x + rel_x;
        let span = Span::styled(padded, row_style);
        frame.buffer_mut().set_span(x, area.y, &span, visible_width);
    }
}

/// Extract text content from a Node (for cell rendering).
fn extract_text_from_node(node: &Node) -> String {
    match node {
        Node::Text { content, .. } => content.clone(),
        Node::Row { children, .. } | Node::Column { children, .. } => {
            children.iter().map(extract_text_from_node).collect()
        }
        _ => String::new(),
    }
}
