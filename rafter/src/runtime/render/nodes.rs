//! Node rendering functions.

use ratatui::Frame;
use ratatui::layout::{Direction, Layout, Rect};
use ratatui::style::{Color, Style as RatatuiStyle};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use super::layout::{apply_border, apply_padding, calculate_constraints};
use super::render_node;
use crate::node::{Layout as NodeLayout, Node};
use crate::runtime::hit_test::HitTestMap;
use crate::theme::Theme;

/// Render text content
pub fn render_text(frame: &mut Frame, content: &str, style: RatatuiStyle, area: Rect) {
    let span = Span::styled(content, style);
    let line = Line::from(span);
    let paragraph = Paragraph::new(line);
    frame.render_widget(paragraph, area);
}

/// Render an input field
pub fn render_input(
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
pub fn render_button(frame: &mut Frame, label: &str, style: RatatuiStyle, focused: bool, area: Rect) {
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
pub fn render_container(
    frame: &mut Frame,
    children: &[Node],
    style: RatatuiStyle,
    layout: &NodeLayout,
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
pub fn render_stack(
    frame: &mut Frame,
    children: &[Node],
    style: RatatuiStyle,
    layout: &NodeLayout,
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
