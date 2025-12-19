//! Rendering - convert Node tree to ratatui widgets.

use std::time::Instant;

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style as RatatuiStyle};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::context::{Toast, ToastLevel};
use crate::node::{Border, Node};

/// Render a Node tree to a ratatui Frame
pub fn render_node(frame: &mut Frame, node: &Node, area: Rect) {
    match node {
        Node::Empty => {}
        Node::Text { content, style } => {
            render_text(frame, content, style.to_ratatui(), area);
        }
        Node::Column {
            children,
            style,
            layout,
        } => {
            render_container(frame, children, style.to_ratatui(), layout, area, false);
        }
        Node::Row {
            children,
            style,
            layout,
        } => {
            render_container(frame, children, style.to_ratatui(), layout, area, true);
        }
        Node::Stack {
            children,
            style,
            layout,
        } => {
            render_stack(frame, children, style.to_ratatui(), layout, area);
        }
        Node::Input {
            value,
            placeholder,
            style,
            focused,
            ..
        } => {
            render_input(
                frame,
                value,
                placeholder,
                style.to_ratatui(),
                *focused,
                area,
            );
        }
        Node::Button {
            label,
            style,
            focused,
            ..
        } => {
            render_button(frame, label, style.to_ratatui(), *focused, area);
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
    let mut button_style = style;
    if focused {
        button_style = button_style.add_modifier(ratatui::style::Modifier::REVERSED);
    }

    let content = format!("[ {} ]", label);
    let paragraph = Paragraph::new(content).style(button_style);
    frame.render_widget(paragraph, area);
}

/// Render a container (column or row)
fn render_container(
    frame: &mut Frame,
    children: &[Node],
    style: RatatuiStyle,
    layout: &crate::node::Layout,
    area: Rect,
    horizontal: bool,
) {
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

    // Calculate constraints for children
    let constraints = calculate_constraints(children, layout.gap);

    // Create layout
    let direction = if horizontal {
        Direction::Horizontal
    } else {
        Direction::Vertical
    };

    let chunks = Layout::default()
        .direction(direction)
        .constraints(constraints)
        .split(padded_area);

    // Render children
    let mut chunk_idx = 0;
    for child in children {
        if chunk_idx < chunks.len() {
            render_node(frame, child, chunks[chunk_idx]);
            chunk_idx += 1;
            // Skip gap chunks
            if layout.gap > 0 && chunk_idx < chunks.len() {
                chunk_idx += 1;
            }
        }
    }
}

/// Render a stack (z-index layering)
fn render_stack(
    frame: &mut Frame,
    children: &[Node],
    style: RatatuiStyle,
    layout: &crate::node::Layout,
    area: Rect,
) {
    // Apply border if specified
    let (inner_area, block) = apply_border(area, &layout.border, style);

    if let Some(block) = block {
        frame.render_widget(block, area);
    }

    // Apply padding
    let padded_area = apply_padding(inner_area, layout.padding);

    // Render all children in the same area (stacked)
    for child in children {
        render_node(frame, child, padded_area);
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
fn calculate_constraints(children: &[Node], gap: u16) -> Vec<Constraint> {
    let mut constraints = Vec::new();

    for (i, child) in children.iter().enumerate() {
        // Add constraint for this child
        constraints.push(child_constraint(child));

        // Add gap between children (except after last)
        if gap > 0 && i < children.len() - 1 {
            constraints.push(Constraint::Length(gap));
        }
    }

    constraints
}

/// Get the constraint for a single child node
fn child_constraint(node: &Node) -> Constraint {
    match node {
        Node::Empty => Constraint::Length(0),
        Node::Text { content, .. } => {
            // Text takes its natural width/height
            let lines: Vec<&str> = content.lines().collect();
            let height = lines.len().max(1) as u16;
            Constraint::Length(height)
        }
        Node::Column { layout, .. } | Node::Row { layout, .. } | Node::Stack { layout, .. } => {
            // Check layout hints
            match layout.height {
                crate::node::Size::Fixed(h) => Constraint::Length(h),
                crate::node::Size::Percent(p) => Constraint::Percentage((p * 100.0) as u16),
                crate::node::Size::Flex(f) => Constraint::Ratio(f as u32, 1),
                crate::node::Size::Auto => {
                    // Auto-size based on flex or equal distribution
                    if let Some(flex) = layout.flex {
                        Constraint::Ratio(flex as u32, 1)
                    } else {
                        Constraint::Min(1)
                    }
                }
            }
        }
        Node::Input { .. } | Node::Button { .. } => {
            // Interactive elements take 1 line
            Constraint::Length(1)
        }
    }
}

/// Render active toasts in the bottom-right corner
pub fn render_toasts(frame: &mut Frame, toasts: &[(Toast, Instant)]) {
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

        // Style based on toast level
        let (border_color, title) = match toast.level {
            ToastLevel::Info => (Color::Blue, "Info"),
            ToastLevel::Success => (Color::Green, "Success"),
            ToastLevel::Warning => (Color::Yellow, "Warning"),
            ToastLevel::Error => (Color::Red, "Error"),
        };

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
