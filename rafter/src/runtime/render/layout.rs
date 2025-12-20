//! Layout calculations for node rendering.

use ratatui::layout::{Constraint, Rect};
use ratatui::style::Style as RatatuiStyle;
use ratatui::widgets::{Block, Borders};

use crate::node::{Border, Node};

/// Constrain area to intrinsic size if layout is auto-sized
pub fn constrain_area(node: &Node, area: Rect) -> Rect {
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

/// Apply border to an area, returning the inner area and optional block widget
pub fn apply_border(
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
pub fn apply_padding(area: Rect, padding: u16) -> Rect {
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
pub fn calculate_constraints(children: &[Node], gap: u16, horizontal: bool) -> Vec<Constraint> {
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
pub fn intrinsic_size(node: &Node, horizontal: bool) -> u16 {
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
        Node::Scrollable { child, layout, .. } => {
            let border_size = if matches!(layout.border, Border::None) {
                0
            } else {
                2
            };
            let padding = layout.padding * 2;
            intrinsic_size(child, horizontal) + padding + border_size
        }
        Node::List { layout, component, .. } => {
            let border_size = if matches!(layout.border, Border::None) {
                0
            } else {
                2
            };
            let padding = layout.padding * 2;
            if horizontal {
                // Width is determined by layout, use a reasonable default
                40 + padding + border_size
            } else {
                // Height is total items height
                component.total_height() + padding + border_size
            }
        }
    }
}

/// Get the constraint for a single child node
pub fn child_constraint(node: &Node, horizontal: bool) -> Constraint {
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
                // Button width: " label " = label + 2
                Constraint::Length((label.len() + 2) as u16)
            } else {
                // For vertical layout, buttons take 1 line
                Constraint::Length(1)
            }
        }
        Node::Scrollable { layout, .. } | Node::List { layout, .. } => {
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
                        Constraint::Length(intrinsic_size(node, horizontal))
                    }
                }
            }
        }
    }
}
