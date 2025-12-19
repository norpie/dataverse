//! Rendering - convert Node tree to ratatui widgets.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style as RatatuiStyle;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

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
    }
}

/// Render text content
fn render_text(frame: &mut Frame, content: &str, style: RatatuiStyle, area: Rect) {
    let span = Span::styled(content, style);
    let line = Line::from(span);
    let paragraph = Paragraph::new(line);
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
    }
}
