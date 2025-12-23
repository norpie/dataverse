//! Container (column/row) widget rendering.

use ratatui::Frame;
use ratatui::layout::{Direction, Flex, Layout, Rect};
use ratatui::style::Style as RatatuiStyle;
use ratatui::widgets::Block;

use crate::layers::overlay::OverlayRequest;
use crate::node::{Justify, Layout as NodeLayout, Node};
use crate::runtime::hit_test::HitTestMap;
use crate::runtime::render::layout::{apply_border, apply_padding, calculate_constraints};
use crate::runtime::render::render_node;
use crate::styling::theme::Theme;

/// Convert our Justify enum to ratatui's Flex enum
fn justify_to_flex(justify: Justify) -> Flex {
    match justify {
        Justify::Start => Flex::Start,
        Justify::Center => Flex::Center,
        Justify::End => Flex::End,
        Justify::SpaceBetween => Flex::SpaceBetween,
        Justify::SpaceAround => Flex::SpaceAround,
    }
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
    overlay_requests: &mut Vec<OverlayRequest>,
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

    // Convert justify to ratatui's Flex
    let flex = justify_to_flex(layout.justify);

    let chunks = Layout::default()
        .direction(direction)
        .flex(flex)
        .constraints(constraints)
        .split(padded_area);

    // Render children
    let mut chunk_idx = 0;
    for child in children {
        if chunk_idx < chunks.len() {
            render_node(
                frame,
                child,
                chunks[chunk_idx],
                hit_map,
                theme,
                focused_id,
                overlay_requests,
            );
            chunk_idx += 1;
            // Skip gap chunks (only when using manual gap, not when using flex spacing)
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
    overlay_requests: &mut Vec<OverlayRequest>,
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
        render_node(
            frame,
            child,
            padded_area,
            hit_map,
            theme,
            focused_id,
            overlay_requests,
        );
    }
}
