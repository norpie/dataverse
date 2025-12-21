//! Rendering - convert Node tree to ratatui widgets.

mod backdrop;
pub(crate) mod layout;
mod primitives;
mod toasts;

use ratatui::Frame;
use ratatui::style::Style as RatatuiStyle;

use super::hit_test::HitTestMap;
use crate::node::Node;
use crate::style::Style;
use crate::theme::{Theme, resolve_color};

pub use backdrop::{dim_backdrop, fill_background};
pub use toasts::render_toasts;

use crate::components::input::render::render_input;
use crate::components::scroll_area::render::{
    ClipRect, calculate_scroll_area_layout, calculate_wrapped_content_size,
    render_horizontal_scrollbar, render_node_clipped, render_vertical_scrollbar,
};
use crate::components::scrollbar::ScrollbarState;
use primitives::{render_button, render_container, render_stack, render_text};

/// Convert a Style to ratatui Style, resolving named colors via theme
pub(crate) fn style_to_ratatui(style: &Style, theme: &dyn Theme) -> RatatuiStyle {
    let mut ratatui_style = RatatuiStyle::default();

    if let Some(ref fg) = style.fg {
        let resolved = resolve_color(fg, theme);
        ratatui_style = ratatui_style.fg(resolved.to_ratatui());
    }

    if let Some(ref bg) = style.bg {
        let resolved = resolve_color(bg, theme);
        ratatui_style = ratatui_style.bg(resolved.to_ratatui());
    }

    if style.bold {
        ratatui_style = ratatui_style.add_modifier(ratatui::style::Modifier::BOLD);
    }

    if style.italic {
        ratatui_style = ratatui_style.add_modifier(ratatui::style::Modifier::ITALIC);
    }

    if style.underline {
        ratatui_style = ratatui_style.add_modifier(ratatui::style::Modifier::UNDERLINED);
    }

    if style.dim {
        ratatui_style = ratatui_style.add_modifier(ratatui::style::Modifier::DIM);
    }

    ratatui_style
}

/// Render a Node tree to a ratatui Frame
pub fn render_node(
    frame: &mut Frame,
    node: &Node,
    area: ratatui::layout::Rect,
    hit_map: &mut HitTestMap,
    theme: &dyn Theme,
    focused_id: Option<&str>,
) {
    // Constrain area for auto-sized containers
    let area = layout::constrain_area(node, area);

    match node {
        Node::Empty => {}
        Node::Text { content, style } => {
            render_text(frame, content, style_to_ratatui(style, theme), area);
        }
        Node::Column {
            children,
            style,
            layout,
        } => {
            render_container(
                frame,
                children,
                style_to_ratatui(style, theme),
                layout,
                area,
                false,
                hit_map,
                theme,
                focused_id,
            );
        }
        Node::Row {
            children,
            style,
            layout,
        } => {
            render_container(
                frame,
                children,
                style_to_ratatui(style, theme),
                layout,
                area,
                true,
                hit_map,
                theme,
                focused_id,
            );
        }
        Node::Stack {
            children,
            style,
            layout,
        } => {
            render_stack(
                frame,
                children,
                style_to_ratatui(style, theme),
                layout,
                area,
                hit_map,
                theme,
                focused_id,
            );
        }
        Node::Input {
            value,
            placeholder,
            style,
            id,
            component,
            ..
        } => {
            let is_focused = focused_id == Some(id.as_str());
            let (display_value, cursor_pos) = component
                .as_ref()
                .map(|c| (c.value(), c.cursor()))
                .unwrap_or_else(|| (value.clone(), value.len()));
            render_input(
                frame,
                &display_value,
                placeholder,
                cursor_pos,
                style_to_ratatui(style, theme),
                is_focused,
                area,
            );
            if !id.is_empty() {
                hit_map.register(id.clone(), area, true);
            }
        }
        Node::Button {
            label, style, id, ..
        } => {
            let is_focused = focused_id == Some(id.as_str());
            render_button(
                frame,
                label,
                style_to_ratatui(style, theme),
                is_focused,
                area,
            );
            if !id.is_empty() {
                hit_map.register(id.clone(), area, false);
            }
        }
        Node::ScrollArea {
            child,
            id,
            style,
            component,
            ..
        } => {
            render_scroll_area(
                frame,
                child,
                id,
                style_to_ratatui(style, theme),
                component,
                area,
                hit_map,
                theme,
                focused_id,
            );
        }
        Node::List {
            id,
            style,
            layout,
            component,
            ..
        } => {
            render_list(
                frame,
                id,
                style_to_ratatui(style, theme),
                layout,
                component.as_ref(),
                area,
                hit_map,
                theme,
                focused_id,
            );
        }
        Node::Tree {
            id,
            style,
            layout,
            component,
            ..
        } => {
            render_tree(
                frame,
                id,
                style_to_ratatui(style, theme),
                layout,
                component.as_ref(),
                area,
                hit_map,
                theme,
                focused_id,
            );
        }
        Node::Table {
            id,
            style,
            layout,
            component,
            ..
        } => {
            render_table(
                frame,
                id,
                style_to_ratatui(style, theme),
                layout,
                component.as_ref(),
                area,
                hit_map,
                theme,
                focused_id,
            );
        }
    }
}

/// Render a scroll area container
#[allow(clippy::too_many_arguments)]
fn render_scroll_area(
    frame: &mut Frame,
    child: &Node,
    id: &str,
    style: RatatuiStyle,
    component: &crate::components::ScrollArea,
    area: ratatui::layout::Rect,
    hit_map: &mut HitTestMap,
    theme: &dyn Theme,
    focused_id: Option<&str>,
) {
    use ratatui::widgets::Block;

    // Fill background if specified
    if style.bg.is_some() {
        let block = Block::default().style(style);
        frame.render_widget(block, area);
    }

    // Calculate layout first to get viewport dimensions
    let initial_content_size = (child.intrinsic_width(), child.intrinsic_height());
    let scroll_layout = calculate_scroll_area_layout(
        area,
        initial_content_size,
        component.direction(),
        &component.scrollbar_config(),
    );

    // Calculate actual content height with wrapping based on viewport width
    let content_size = calculate_wrapped_content_size(child, scroll_layout.content_area.width);

    // Update component with computed sizes
    component.set_sizes(
        content_size,
        (
            scroll_layout.content_area.width,
            scroll_layout.content_area.height,
        ),
    );

    // Get scroll offset
    let (offset_x, offset_y) = component.offset();

    // Render scrollbars and save geometry for hit testing
    let v_geom = if scroll_layout.show_vertical {
        render_vertical_scrollbar(
            frame.buffer_mut(),
            area,
            offset_y,
            content_size.1,
            scroll_layout.content_area.height,
            &component.scrollbar_config(),
            theme,
        )
    } else {
        None
    };
    component.set_vertical_scrollbar(v_geom);

    let h_geom = if scroll_layout.show_horizontal {
        render_horizontal_scrollbar(
            frame.buffer_mut(),
            area,
            offset_x,
            content_size.0,
            scroll_layout.content_area.width,
            &component.scrollbar_config(),
            theme,
        )
    } else {
        None
    };
    component.set_horizontal_scrollbar(h_geom);

    // Render child with viewport clipping
    let viewport = scroll_layout.content_area;

    if viewport.width > 0 && viewport.height > 0 {
        let clip = ClipRect {
            viewport,
            offset_x,
            offset_y,
        };

        render_node_clipped(
            frame,
            child,
            viewport,
            &clip,
            hit_map,
            theme,
            focused_id,
            style_to_ratatui,
            render_node,
        );
    }

    // Register hit box for scroll area (focusable for keyboard navigation)
    if !id.is_empty() {
        hit_map.register(id.to_string(), area, true);
    }
}

/// Render a list component
#[allow(clippy::too_many_arguments)]
fn render_list(
    frame: &mut Frame,
    id: &str,
    style: RatatuiStyle,
    layout: &crate::node::Layout,
    component: &dyn crate::components::list::AnyList,
    area: ratatui::layout::Rect,
    hit_map: &mut HitTestMap,
    theme: &dyn Theme,
    focused_id: Option<&str>,
) {
    use crate::components::scrollbar::render_vertical_scrollbar;
    use ratatui::widgets::Block;

    // Apply border and get inner area
    let (inner_area, block) =
        crate::runtime::render::layout::apply_border(area, &layout.border, style);
    if let Some(block) = block {
        frame.render_widget(block, area);
    } else if style.bg.is_some() {
        // Fill background if no border but has background
        let bg_block = Block::default().style(style);
        frame.render_widget(bg_block, area);
    }

    // Apply padding
    let padded_area = crate::runtime::render::layout::apply_padding(inner_area, layout.padding);

    if padded_area.width == 0 || padded_area.height == 0 {
        return;
    }

    // Determine if we need a scrollbar
    let needs_scrollbar = component.needs_vertical_scrollbar();
    // Scrollbar takes 1 cell, plus 1 cell padding between content and scrollbar
    let scrollbar_reserved = if needs_scrollbar { 2u16 } else { 0u16 };

    // Content area excludes scrollbar and padding
    let content_area = ratatui::layout::Rect {
        x: padded_area.x,
        y: padded_area.y,
        width: padded_area.width.saturating_sub(scrollbar_reserved),
        height: padded_area.height,
    };

    if content_area.width == 0 || content_area.height == 0 {
        return;
    }

    // Update component's viewport height
    component.set_viewport_height(content_area.height);

    // Get visible range
    let visible_range = component.visible_range();
    let item_height = component.item_height();
    let scroll_offset = component.scroll_offset();

    // Calculate offset for first visible item
    let first_item_y = (visible_range.start as u16 * item_height).saturating_sub(scroll_offset);

    // Render visible items
    for (i, index) in visible_range.enumerate() {
        let item_y = content_area.y + first_item_y + (i as u16 * item_height);

        // Skip if outside viewport
        if item_y >= content_area.y + content_area.height {
            break;
        }

        let item_area = ratatui::layout::Rect {
            x: content_area.x,
            y: item_y,
            width: content_area.width,
            height: item_height.min(content_area.y + content_area.height - item_y),
        };

        // Render the item
        if let Some(item_node) = component.render_item(index) {
            render_node(frame, &item_node, item_area, hit_map, theme, focused_id);
        }
    }

    // Render vertical scrollbar if needed
    if needs_scrollbar {
        let scrollbar_area = ratatui::layout::Rect {
            x: padded_area.x + padded_area.width - 1,
            y: padded_area.y,
            width: 1,
            height: padded_area.height,
        };

        let config = component.scrollbar_config();
        let v_geom = render_vertical_scrollbar(
            frame.buffer_mut(),
            scrollbar_area,
            scroll_offset,
            component.total_height(),
            content_area.height,
            &config,
            theme,
        );
        component.set_vertical_scrollbar(v_geom);
    } else {
        component.set_vertical_scrollbar(None);
    }

    // Register hit box for the content area (where items are clickable)
    // We use padded_area so click coordinates are relative to item positions
    if !id.is_empty() {
        hit_map.register(id.to_string(), padded_area, true);
    }
}

/// Render a tree component
#[allow(clippy::too_many_arguments)]
fn render_tree(
    frame: &mut Frame,
    id: &str,
    style: RatatuiStyle,
    layout: &crate::node::Layout,
    component: &dyn crate::components::tree::AnyTree,
    area: ratatui::layout::Rect,
    hit_map: &mut HitTestMap,
    theme: &dyn Theme,
    focused_id: Option<&str>,
) {
    use crate::components::scrollbar::render_vertical_scrollbar;
    use ratatui::widgets::Block;

    // Apply border and get inner area
    let (inner_area, block) =
        crate::runtime::render::layout::apply_border(area, &layout.border, style);
    if let Some(block) = block {
        frame.render_widget(block, area);
    } else if style.bg.is_some() {
        // Fill background if no border but has background
        let bg_block = Block::default().style(style);
        frame.render_widget(bg_block, area);
    }

    // Apply padding
    let padded_area = crate::runtime::render::layout::apply_padding(inner_area, layout.padding);

    if padded_area.width == 0 || padded_area.height == 0 {
        return;
    }

    // Determine if we need a scrollbar
    let needs_scrollbar = component.needs_vertical_scrollbar();
    let scrollbar_reserved = if needs_scrollbar { 2u16 } else { 0u16 };

    // Content area excludes scrollbar and padding
    let content_area = ratatui::layout::Rect {
        x: padded_area.x,
        y: padded_area.y,
        width: padded_area.width.saturating_sub(scrollbar_reserved),
        height: padded_area.height,
    };

    if content_area.width == 0 || content_area.height == 0 {
        return;
    }

    // Update component's viewport height
    component.set_viewport_height(content_area.height);

    // Get visible range
    let visible_range = component.visible_range();
    let item_height = component.item_height();
    let scroll_offset = component.scroll_offset();

    // Calculate offset for first visible item
    let first_item_y = (visible_range.start as u16 * item_height).saturating_sub(scroll_offset);

    // Render visible nodes
    for (i, index) in visible_range.enumerate() {
        let item_y = content_area.y + first_item_y + (i as u16 * item_height);

        // Skip if outside viewport
        if item_y >= content_area.y + content_area.height {
            break;
        }

        let item_area = ratatui::layout::Rect {
            x: content_area.x,
            y: item_y,
            width: content_area.width,
            height: item_height.min(content_area.y + content_area.height - item_y),
        };

        // Render the node
        if let Some(item_node) = component.render_item(index) {
            render_node(frame, &item_node, item_area, hit_map, theme, focused_id);
        }
    }

    // Render vertical scrollbar if needed
    if needs_scrollbar {
        let scrollbar_area = ratatui::layout::Rect {
            x: padded_area.x + padded_area.width - 1,
            y: padded_area.y,
            width: 1,
            height: padded_area.height,
        };

        let config = component.scrollbar_config();
        let v_geom = render_vertical_scrollbar(
            frame.buffer_mut(),
            scrollbar_area,
            scroll_offset,
            component.total_height(),
            content_area.height,
            &config,
            theme,
        );
        component.set_vertical_scrollbar(v_geom);
    } else {
        component.set_vertical_scrollbar(None);
    }

    // Register hit box for the content area (where nodes are clickable)
    if !id.is_empty() {
        hit_map.register(id.to_string(), padded_area, true);
    }
}

/// Render a table component
#[allow(clippy::too_many_arguments)]
fn render_table(
    frame: &mut Frame,
    id: &str,
    style: RatatuiStyle,
    layout: &crate::node::Layout,
    component: &dyn crate::components::table::AnyTable,
    area: ratatui::layout::Rect,
    hit_map: &mut HitTestMap,
    theme: &dyn Theme,
    _focused_id: Option<&str>,
) {
    use crate::components::scrollbar::{render_horizontal_scrollbar, render_vertical_scrollbar};
    use ratatui::widgets::Block;

    // Apply border and get inner area
    let (inner_area, block) =
        crate::runtime::render::layout::apply_border(area, &layout.border, style);
    if let Some(block) = block {
        frame.render_widget(block, area);
    } else if style.bg.is_some() {
        // Fill background if no border but has background
        let bg_block = Block::default().style(style);
        frame.render_widget(bg_block, area);
    }

    // Apply padding
    let padded_area = crate::runtime::render::layout::apply_padding(inner_area, layout.padding);

    if padded_area.width == 0 || padded_area.height == 0 {
        return;
    }

    // Calculate effective table width (clamped to content)
    let table_content_width = component.total_width();
    
    // First pass: estimate viewport to determine scrollbar needs
    // Use the smaller of available space and content size
    let estimated_width = padded_area.width.min(table_content_width);
    let estimated_height = padded_area.height;
    
    component.set_viewport_width(estimated_width);
    component.set_viewport_height(estimated_height);

    // Determine scrollbar needs based on content vs viewport
    let needs_v_scrollbar = component.needs_vertical_scrollbar();
    let needs_h_scrollbar = component.needs_horizontal_scrollbar();

    // Reserve space for scrollbars
    let v_scrollbar_reserved = if needs_v_scrollbar { 2u16 } else { 0u16 };
    let h_scrollbar_reserved = if needs_h_scrollbar { 1u16 } else { 0u16 };

    // Content area excludes scrollbars
    let content_area = ratatui::layout::Rect {
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
    component.set_viewport_height(content_area.height);
    component.set_viewport_width(effective_width);

    // Use effective width for rendering (clamped to content)
    let render_area = ratatui::layout::Rect {
        x: content_area.x,
        y: content_area.y,
        width: effective_width,
        height: content_area.height,
    };

    // Header is at row 0, data starts at row 1
    let header_height = 1u16;
    let data_area = ratatui::layout::Rect {
        x: render_area.x,
        y: render_area.y + header_height,
        width: render_area.width,
        height: render_area.height.saturating_sub(header_height),
    };

    let columns = component.columns();
    let scroll_offset_x = component.scroll_offset_x();
    let scroll_offset_y = component.scroll_offset_y();
    let visible_col_range = component.visible_column_range();
    let visible_row_range = component.visible_row_range();
    let row_height = component.row_height();

    // Render header row
    render_table_header(
        frame,
        &columns,
        component.sort(),
        ratatui::layout::Rect {
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

        let row_area = ratatui::layout::Rect {
            x: data_area.x,
            y: row_y,
            width: data_area.width,
            height: row_height.min(data_area.y + data_area.height - row_y),
        };

        // Render the row with cell-by-cell column widths
        render_table_row(
            frame,
            component,
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
        let scrollbar_area = ratatui::layout::Rect {
            x: render_area.x + render_area.width,
            y: render_area.y,
            width: 1,
            height: render_area.height.saturating_sub(h_scrollbar_reserved),
        };

        let config = component.scrollbar_config();
        let v_geom = render_vertical_scrollbar(
            frame.buffer_mut(),
            scrollbar_area,
            scroll_offset_y,
            component.total_height(),
            component.data_viewport_height(),
            &config,
            theme,
        );
        component.set_vertical_scrollbar(v_geom);
    } else {
        component.set_vertical_scrollbar(None);
    }

    // Render horizontal scrollbar if needed
    if needs_h_scrollbar {
        let scrollbar_area = ratatui::layout::Rect {
            x: render_area.x,
            y: render_area.y + render_area.height,
            width: render_area.width + v_scrollbar_reserved,
            height: 1,
        };

        let config = component.scrollbar_config();
        let h_geom = render_horizontal_scrollbar(
            frame.buffer_mut(),
            scrollbar_area,
            scroll_offset_x,
            component.total_width(),
            content_area.width,
            &config,
            theme,
        );
        component.set_horizontal_scrollbar(h_geom);
    } else {
        component.set_horizontal_scrollbar(None);
    }

    // Register hit box for the content area
    if !id.is_empty() {
        hit_map.register(id.to_string(), padded_area, true);
    }
}

/// Render the table header row
fn render_table_header(
    frame: &mut Frame,
    columns: &[crate::components::table::Column],
    sort: Option<(usize, bool)>,
    area: ratatui::layout::Rect,
    scroll_offset_x: u16,
    visible_col_range: std::ops::Range<usize>,
    theme: &dyn Theme,
) {
    use ratatui::style::{Modifier, Style as RStyle};
    use ratatui::text::Span;

    // Get theme colors for header
    let header_bg = theme
        .resolve("surface")
        .unwrap_or(crate::color::Color::hex(0x1e1e2e));
    let header_fg = theme
        .resolve("text")
        .unwrap_or(crate::color::Color::hex(0xcdd6f4));
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
                crate::components::table::Alignment::Left => {
                    // Indicator on right
                    format!("{} {}", col.header, indicator)
                }
                crate::components::table::Alignment::Right => {
                    // Indicator on left
                    format!("{} {}", indicator, col.header)
                }
                crate::components::table::Alignment::Center => {
                    // Check which side has more room
                    let header_len = col.header.chars().count();
                    let col_width = col.width as usize;
                    let left_padding = col_width.saturating_sub(header_len) / 2;
                    let right_padding = col_width.saturating_sub(header_len).saturating_sub(left_padding);
                    
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
            crate::components::table::Alignment::Left => {
                format!("{:<width$}", header_text, width = col.width as usize)
            }
            crate::components::table::Alignment::Center => {
                format!("{:^width$}", header_text, width = col.width as usize)
            }
            crate::components::table::Alignment::Right => {
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

    // Fill any remaining space with background
    // (This handles the case where columns don't fill the viewport)
}

/// Render a single table data row with proper column alignment.
#[allow(clippy::too_many_arguments)]
fn render_table_row(
    frame: &mut Frame,
    component: &dyn crate::components::table::AnyTable,
    row_index: usize,
    columns: &[crate::components::table::Column],
    col_positions: &[u16],
    scroll_offset_x: u16,
    visible_col_range: std::ops::Range<usize>,
    area: ratatui::layout::Rect,
    theme: &dyn Theme,
) {
    use crate::components::table::Alignment;
    use ratatui::style::Style as RStyle;
    use ratatui::text::Span;

    let is_focused = component.is_focused_at(row_index);
    let is_selected = component.is_selected_at(row_index);

    // Determine row background based on focus/selection state
    let row_style = if is_focused {
        // Bright purple for cursor
        let bg = crate::color::Color::hex(0xA277FF);
        let fg = theme
            .resolve("background")
            .unwrap_or(crate::color::Color::hex(0x1e1e2e));
        RStyle::default()
            .bg(ratatui::style::Color::Rgb(bg.r(), bg.g(), bg.b()))
            .fg(ratatui::style::Color::Rgb(fg.r(), fg.g(), fg.b()))
    } else if is_selected {
        // Dimmer purple for selected
        let bg = crate::color::Color::hex(0x6E5494);
        let fg = theme
            .resolve("background")
            .unwrap_or(crate::color::Color::hex(0x1e1e2e));
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
        let cell_text = if let Some(cell_node) = component.render_cell(row_index, col_idx) {
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
fn extract_text_from_node(node: &crate::node::Node) -> String {
    match node {
        crate::node::Node::Text { content, .. } => content.clone(),
        crate::node::Node::Row { children, .. } | crate::node::Node::Column { children, .. } => {
            children.iter().map(extract_text_from_node).collect()
        }
        _ => String::new(),
    }
}
