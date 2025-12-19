//! Rendering - convert Node tree to ratatui widgets.

use std::time::Instant;

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style as RatatuiStyle};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use super::hit_test::HitTestMap;
use crate::context::{Toast, ToastLevel};
use crate::node::{Border, Node};
use crate::style::Style;
use crate::theme::{Theme, resolve_color};

/// Convert a Style to ratatui Style, resolving named colors via theme
fn style_to_ratatui(style: &Style, theme: &dyn Theme) -> RatatuiStyle {
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
/// Constrain area to intrinsic size if layout is auto-sized
fn constrain_area(node: &Node, area: Rect) -> Rect {
    let layout = match node {
        Node::Column { layout, .. } | Node::Row { layout, .. } | Node::Stack { layout, .. } => {
            layout
        }
        _ => return area,
    };

    let mut result = area;

    // Constrain width if auto
    if matches!(layout.width, crate::node::Size::Auto) && layout.flex.is_none() {
        let intrinsic_w = intrinsic_size(node, true);
        result.width = result.width.min(intrinsic_w);
    }

    // Constrain height if auto
    if matches!(layout.height, crate::node::Size::Auto) && layout.flex.is_none() {
        let intrinsic_h = intrinsic_size(node, false);
        result.height = result.height.min(intrinsic_h);
    }

    result
}

pub fn render_node(
    frame: &mut Frame,
    node: &Node,
    area: Rect,
    hit_map: &mut HitTestMap,
    theme: &dyn Theme,
    focused_id: Option<&str>,
) {
    // Constrain area for auto-sized containers
    let area = constrain_area(node, area);

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
            ..
        } => {
            let is_focused = focused_id == Some(id.as_str());
            render_input(
                frame,
                value,
                placeholder,
                style_to_ratatui(style, theme),
                is_focused,
                area,
            );
            // Register hit box for input
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
            // Register hit box for button
            if !id.is_empty() {
                hit_map.register(id.clone(), area, false);
            }
        }
    }
}

/// Render text content
fn render_text(frame: &mut Frame, content: &str, style: RatatuiStyle, area: Rect) {
    let span = Span::styled(content, style);
    let line = Line::from(span);
    let paragraph = Paragraph::new(line);
    frame.render_widget(paragraph, area);
}

/// Render an input field
fn render_input(
    frame: &mut Frame,
    value: &str,
    placeholder: &str,
    style: RatatuiStyle,
    focused: bool,
    area: Rect,
) {
    let display_text = if value.is_empty() { placeholder } else { value };

    let mut input_style = style;
    if focused {
        input_style = input_style.add_modifier(ratatui::style::Modifier::REVERSED);
    }

    // Show cursor at end if focused
    let content = if focused {
        format!("{}█", display_text)
    } else {
        display_text.to_string()
    };

    let paragraph = Paragraph::new(content).style(input_style);
    frame.render_widget(paragraph, area);
}

/// Render a button
fn render_button(frame: &mut Frame, label: &str, style: RatatuiStyle, focused: bool, area: Rect) {
    // Buttons have a subtle background, brighter when focused
    let button_style = if focused {
        style
            .bg(Color::Rgb(80, 80, 100))
            .add_modifier(ratatui::style::Modifier::BOLD)
    } else {
        style.bg(Color::Rgb(50, 50, 65))
    };

    // Add padding with spaces
    let content = format!(" {} ", label);
    let paragraph = Paragraph::new(content).style(button_style);
    frame.render_widget(paragraph, area);
}

/// Render a container (column or row)
#[allow(clippy::too_many_arguments)]
fn render_container(
    frame: &mut Frame,
    children: &[Node],
    style: RatatuiStyle,
    layout: &crate::node::Layout,
    area: Rect,
    horizontal: bool,
    hit_map: &mut HitTestMap,
    theme: &dyn Theme,
    focused_id: Option<&str>,
) {
    // Fill background if specified
    if style.bg.is_some() {
        let block = Block::default().style(style);
        frame.render_widget(block, area);
    }

    if children.is_empty() {
        return;
    }

    // Apply border if specified
    let (inner_area, block) = apply_border(area, &layout.border, style);

    if let Some(block) = block {
        frame.render_widget(block, area);
    }

    // Apply padding
    let padded_area = apply_padding(inner_area, layout.padding);

    // Create layout
    let direction = if horizontal {
        Direction::Horizontal
    } else {
        Direction::Vertical
    };

    // Calculate constraints for children
    let constraints = calculate_constraints(children, layout.gap, horizontal);

    let chunks = Layout::default()
        .direction(direction)
        .constraints(constraints)
        .split(padded_area);

    // Render children
    let mut chunk_idx = 0;
    for child in children {
        if chunk_idx < chunks.len() {
            render_node(frame, child, chunks[chunk_idx], hit_map, theme, focused_id);
            chunk_idx += 1;
            // Skip gap chunks
            if layout.gap > 0 && chunk_idx < chunks.len() {
                chunk_idx += 1;
            }
        }
    }
}

/// Render a stack (z-index layering)
#[allow(clippy::too_many_arguments)]
fn render_stack(
    frame: &mut Frame,
    children: &[Node],
    style: RatatuiStyle,
    layout: &crate::node::Layout,
    area: Rect,
    hit_map: &mut HitTestMap,
    theme: &dyn Theme,
    focused_id: Option<&str>,
) {
    // Fill background if specified
    if style.bg.is_some() {
        let block = Block::default().style(style);
        frame.render_widget(block, area);
    }

    // Apply border if specified
    let (inner_area, block) = apply_border(area, &layout.border, style);

    if let Some(block) = block {
        frame.render_widget(block, area);
    }

    // Apply padding
    let padded_area = apply_padding(inner_area, layout.padding);

    // Render all children in the same area (stacked)
    for child in children {
        render_node(frame, child, padded_area, hit_map, theme, focused_id);
    }
}

/// Apply border to an area, returning the inner area and optional block widget
fn apply_border(
    area: Rect,
    border: &Border,
    style: RatatuiStyle,
) -> (Rect, Option<Block<'static>>) {
    match border {
        Border::None => (area, None),
        Border::Single => {
            let block = Block::default().borders(Borders::ALL).style(style);
            let inner = block.inner(area);
            (inner, Some(block))
        }
        Border::Double => {
            let block = Block::default()
                .borders(Borders::ALL)
                .border_type(ratatui::widgets::BorderType::Double)
                .style(style);
            let inner = block.inner(area);
            (inner, Some(block))
        }
        Border::Rounded => {
            let block = Block::default()
                .borders(Borders::ALL)
                .border_type(ratatui::widgets::BorderType::Rounded)
                .style(style);
            let inner = block.inner(area);
            (inner, Some(block))
        }
        Border::Thick => {
            let block = Block::default()
                .borders(Borders::ALL)
                .border_type(ratatui::widgets::BorderType::Thick)
                .style(style);
            let inner = block.inner(area);
            (inner, Some(block))
        }
    }
}

/// Apply padding to an area
fn apply_padding(area: Rect, padding: u16) -> Rect {
    if padding == 0 || area.width < padding * 2 || area.height < padding * 2 {
        return area;
    }

    Rect::new(
        area.x + padding,
        area.y + padding,
        area.width.saturating_sub(padding * 2),
        area.height.saturating_sub(padding * 2),
    )
}

/// Calculate layout constraints for children
fn calculate_constraints(children: &[Node], gap: u16, horizontal: bool) -> Vec<Constraint> {
    let mut constraints = Vec::new();

    for (i, child) in children.iter().enumerate() {
        // Add constraint for this child
        constraints.push(child_constraint(child, horizontal));

        // Add gap between children (except after last)
        if gap > 0 && i < children.len() - 1 {
            constraints.push(Constraint::Length(gap));
        }
    }

    constraints
}

/// Calculate the intrinsic size of a node (width if horizontal, height if vertical)
fn intrinsic_size(node: &Node, horizontal: bool) -> u16 {
    match node {
        Node::Empty => 0,
        Node::Text { content, .. } => {
            if horizontal {
                content.len() as u16
            } else {
                content.lines().count().max(1) as u16
            }
        }
        Node::Column {
            children, layout, ..
        } => {
            let border_size = if matches!(layout.border, Border::None) {
                0
            } else {
                2
            };
            let padding = layout.padding * 2;
            if horizontal {
                // Width: max child width + padding + border
                let max_child = children
                    .iter()
                    .map(|c| intrinsic_size(c, true))
                    .max()
                    .unwrap_or(0);
                max_child + padding + border_size
            } else {
                // Height: sum of children + gaps + padding + border
                let child_sum: u16 = children.iter().map(|c| intrinsic_size(c, false)).sum();
                let gaps = if children.len() > 1 {
                    layout.gap * (children.len() as u16 - 1)
                } else {
                    0
                };
                child_sum + gaps + padding + border_size
            }
        }
        Node::Row {
            children, layout, ..
        } => {
            let border_size = if matches!(layout.border, Border::None) {
                0
            } else {
                2
            };
            let padding = layout.padding * 2;
            if horizontal {
                // Width: sum of children + gaps + padding + border
                let child_sum: u16 = children.iter().map(|c| intrinsic_size(c, true)).sum();
                let gaps = if children.len() > 1 {
                    layout.gap * (children.len() as u16 - 1)
                } else {
                    0
                };
                child_sum + gaps + padding + border_size
            } else {
                // Height: max child height + padding + border
                let max_child = children
                    .iter()
                    .map(|c| intrinsic_size(c, false))
                    .max()
                    .unwrap_or(0);
                max_child + padding + border_size
            }
        }
        Node::Stack {
            children, layout, ..
        } => {
            let border_size = if matches!(layout.border, Border::None) {
                0
            } else {
                2
            };
            let padding = layout.padding * 2;
            // Stack: max of all children in both directions
            let max_child = children
                .iter()
                .map(|c| intrinsic_size(c, horizontal))
                .max()
                .unwrap_or(0);
            max_child + padding + border_size
        }
        Node::Input {
            value, placeholder, ..
        } => {
            if horizontal {
                let content_len = if value.is_empty() {
                    placeholder.len()
                } else {
                    value.len()
                };
                (content_len + 5).max(15) as u16
            } else {
                1
            }
        }
        Node::Button { label, .. } => {
            if horizontal {
                (label.len() + 2) as u16 // " label " with padding
            } else {
                1
            }
        }
    }
}

/// Get the constraint for a single child node
fn child_constraint(node: &Node, horizontal: bool) -> Constraint {
    match node {
        Node::Empty => Constraint::Length(0),
        Node::Text { content, .. } => {
            if horizontal {
                let width = content.len() as u16;
                Constraint::Length(width)
            } else {
                let height = content.lines().count().max(1) as u16;
                Constraint::Length(height)
            }
        }
        Node::Column { layout, .. } | Node::Row { layout, .. } | Node::Stack { layout, .. } => {
            let size = if horizontal {
                &layout.width
            } else {
                &layout.height
            };
            match size {
                crate::node::Size::Fixed(v) => Constraint::Length(*v),
                crate::node::Size::Percent(p) => Constraint::Percentage((*p * 100.0) as u16),
                crate::node::Size::Flex(f) => Constraint::Ratio(*f as u32, 1),
                crate::node::Size::Auto => {
                    if let Some(flex) = layout.flex {
                        Constraint::Ratio(flex as u32, 1)
                    } else {
                        // Calculate intrinsic size
                        Constraint::Length(intrinsic_size(node, horizontal))
                    }
                }
            }
        }
        Node::Input {
            value, placeholder, ..
        } => {
            if horizontal {
                let content_len = if value.is_empty() {
                    placeholder.len()
                } else {
                    value.len()
                };
                Constraint::Min((content_len + 5).max(15) as u16)
            } else {
                Constraint::Length(1)
            }
        }
        Node::Button { label, .. } => {
            if horizontal {
                // Button width: "[ label ]" = label + 4
                Constraint::Length((label.len() + 2) as u16)
            } else {
                // For vertical layout, buttons take 1 line
                Constraint::Length(1)
            }
        }
    }
}

/// Fill the entire buffer with a background color.
///
/// This should be called before rendering the view to ensure
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

/// Render active toasts in the bottom-right corner
pub fn render_toasts(frame: &mut Frame, toasts: &[(Toast, Instant)], theme: &dyn Theme) {
    if toasts.is_empty() {
        return;
    }

    let area = frame.area();

    // Calculate toast dimensions
    const TOAST_WIDTH: u16 = 40;
    const TOAST_HEIGHT: u16 = 3;
    const TOAST_MARGIN: u16 = 1;

    // Render toasts from bottom to top
    for (i, (toast, _expiry)) in toasts.iter().enumerate().take(5) {
        let y_offset = (i as u16) * (TOAST_HEIGHT + TOAST_MARGIN);

        // Position in bottom-right corner
        let toast_area = Rect::new(
            area.width.saturating_sub(TOAST_WIDTH + 2),
            area.height.saturating_sub(TOAST_HEIGHT + 2 + y_offset),
            TOAST_WIDTH,
            TOAST_HEIGHT,
        );

        // Skip if toast would be off-screen
        if toast_area.y == 0 || toast_area.x == 0 {
            continue;
        }

        // Get border color from theme based on toast level
        let (theme_color_name, title) = match toast.level {
            ToastLevel::Info => ("info", "Info"),
            ToastLevel::Success => ("success", "Success"),
            ToastLevel::Warning => ("warning", "Warning"),
            ToastLevel::Error => ("error", "Error"),
        };

        let border_color = theme
            .resolve(theme_color_name)
            .map(|c| c.to_ratatui())
            .unwrap_or(Color::White);

        // Clear the area first (so toasts appear on top)
        frame.render_widget(Clear, toast_area);

        // Create toast block
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Rounded)
            .border_style(RatatuiStyle::default().fg(border_color))
            .title(title);

        let inner = block.inner(toast_area);
        frame.render_widget(block, toast_area);

        // Render message (truncate if needed)
        let max_width = inner.width as usize;
        let message = if toast.message.len() > max_width {
            format!("{}...", &toast.message[..max_width.saturating_sub(3)])
        } else {
            toast.message.clone()
        };

        let paragraph = Paragraph::new(message);
        frame.render_widget(paragraph, inner);
    }
}
