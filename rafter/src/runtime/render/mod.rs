//! Rendering - convert Node tree to ratatui widgets.

mod backdrop;
mod primitives;
mod layout;
mod toasts;

use ratatui::Frame;
use ratatui::style::Style as RatatuiStyle;

use super::hit_test::HitTestMap;
use crate::node::Node;
use crate::style::Style;
use crate::theme::{resolve_color, Theme};

pub use backdrop::{dim_backdrop, fill_background};
pub use toasts::render_toasts;

use crate::components::input::render::render_input;
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
            widget,
            ..
        } => {
            let is_focused = focused_id == Some(id.as_str());
            // If widget is present, read value from it (it's the source of truth)
            // Otherwise fall back to the node's value
            let display_value = widget
                .as_ref()
                .map(|w| w.value())
                .unwrap_or_else(|| value.clone());
            render_input(
                frame,
                &display_value,
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
        Node::Scrollable {
            child,
            id,
            style,
            layout,
            widget,
        } => {
            render_scrollable(
                frame,
                child,
                id,
                style_to_ratatui(style, theme),
                layout,
                widget,
                area,
                hit_map,
                theme,
                focused_id,
            );
        }
    }
}

/// Render a scrollable container
#[allow(clippy::too_many_arguments)]
fn render_scrollable(
    frame: &mut Frame,
    child: &Node,
    id: &str,
    style: RatatuiStyle,
    _layout: &crate::node::Layout,
    widget: &crate::components::Scrollable,
    area: ratatui::layout::Rect,
    hit_map: &mut HitTestMap,
    theme: &dyn Theme,
    focused_id: Option<&str>,
) {
    use crate::components::scrollable::render::{
        calculate_scrollable_layout, render_horizontal_scrollbar, render_vertical_scrollbar,
    };
    use ratatui::widgets::Block;

    // Fill background if specified
    if style.bg.is_some() {
        let block = Block::default().style(style);
        frame.render_widget(block, area);
    }

    // Get content intrinsic size
    let content_size = (child.intrinsic_width(), child.intrinsic_height());

    // Calculate layout (determines scrollbar visibility and content area)
    let scroll_layout = calculate_scrollable_layout(
        area,
        content_size,
        widget.direction(),
        &widget.scrollbar_config(),
    );

    // Update widget with computed sizes
    widget.set_sizes(
        content_size,
        (scroll_layout.content_area.width, scroll_layout.content_area.height),
    );

    // Get scroll offset
    let (offset_x, offset_y) = widget.offset();

    // Render scrollbars
    if scroll_layout.show_vertical {
        render_vertical_scrollbar(
            frame.buffer_mut(),
            area,
            offset_y,
            content_size.1,
            scroll_layout.content_area.height,
            &widget.scrollbar_config(),
            theme,
        );
    }

    if scroll_layout.show_horizontal {
        render_horizontal_scrollbar(
            frame.buffer_mut(),
            area,
            offset_x,
            content_size.0,
            scroll_layout.content_area.width,
            &widget.scrollbar_config(),
            theme,
        );
    }

    // Render child with viewport clipping
    // Instead of rendering to a full buffer and copying, we render directly
    // but clip to the viewport area
    let viewport = scroll_layout.content_area;

    if viewport.width > 0 && viewport.height > 0 {
        // Create a clipping context for the viewport
        let clip = ClipRect {
            viewport,
            offset_x,
            offset_y,
        };
        
        // Render child with clipping - position it as if scrolled
        render_node_clipped(frame, child, viewport, &clip, hit_map, theme, focused_id);
    }

    // Register hit box for scroll area
    if !id.is_empty() {
        hit_map.register(id.to_string(), area, false);
    }
}

/// Clipping context for scrollable viewports
struct ClipRect {
    viewport: ratatui::layout::Rect,
    offset_x: u16,
    offset_y: u16,
}

/// Render a node with viewport clipping (for scrollable content)
fn render_node_clipped(
    frame: &mut Frame,
    node: &Node,
    area: ratatui::layout::Rect,
    clip: &ClipRect,
    hit_map: &mut HitTestMap,
    theme: &dyn Theme,
    focused_id: Option<&str>,
) {
    // For text nodes, we need special handling to show only visible lines
    match node {
        Node::Empty => {}
        Node::Text { content, style } => {
            render_text_clipped(frame, content, style_to_ratatui(style, theme), area, clip);
        }
        Node::Column {
            children,
            style,
            layout: node_layout,
        } => {
            render_container_clipped(
                frame,
                children,
                style_to_ratatui(style, theme),
                node_layout,
                area,
                false,
                clip,
                hit_map,
                theme,
                focused_id,
            );
        }
        Node::Row {
            children,
            style,
            layout: node_layout,
        } => {
            render_container_clipped(
                frame,
                children,
                style_to_ratatui(style, theme),
                node_layout,
                area,
                true,
                clip,
                hit_map,
                theme,
                focused_id,
            );
        }
        Node::Stack {
            children,
            style,
            layout: node_layout,
        } => {
            // For stack, render all children clipped
            let ratatui_style = style_to_ratatui(style, theme);
            if ratatui_style.bg.is_some() {
                let block = ratatui::widgets::Block::default().style(ratatui_style);
                frame.render_widget(block, area);
            }
            let (inner_area, block) = layout::apply_border(area, &node_layout.border, ratatui_style);
            if let Some(block) = block {
                frame.render_widget(block, area);
            }
            let padded_area = layout::apply_padding(inner_area, node_layout.padding);
            for child in children {
                render_node_clipped(frame, child, padded_area, clip, hit_map, theme, focused_id);
            }
        }
        // For other node types, fall back to regular rendering if in viewport
        _ => {
            render_node(frame, node, area, hit_map, theme, focused_id);
        }
    }
}

/// Render text with vertical clipping (skip lines above viewport, stop at bottom)
fn render_text_clipped(
    frame: &mut Frame,
    content: &str,
    style: RatatuiStyle,
    _area: ratatui::layout::Rect,
    clip: &ClipRect,
) {
    use ratatui::text::Line;
    use ratatui::widgets::Paragraph;

    // Split content into lines
    let lines: Vec<&str> = content.lines().collect();
    let total_lines = lines.len();
    
    // Calculate which lines are visible
    let start_line = clip.offset_y as usize;
    let visible_lines = clip.viewport.height as usize;
    let end_line = (start_line + visible_lines).min(total_lines);
    
    if start_line >= total_lines {
        return; // Nothing to render
    }
    
    // Get only the visible lines
    let visible: Vec<Line> = lines[start_line..end_line]
        .iter()
        .map(|&s| Line::raw(s))
        .collect();
    
    let paragraph = Paragraph::new(visible).style(style);
    frame.render_widget(paragraph, clip.viewport);
}

/// Render a container with clipping
#[allow(clippy::too_many_arguments)]
fn render_container_clipped(
    frame: &mut Frame,
    children: &[Node],
    style: RatatuiStyle,
    node_layout: &crate::node::Layout,
    area: ratatui::layout::Rect,
    horizontal: bool,
    clip: &ClipRect,
    hit_map: &mut HitTestMap,
    theme: &dyn Theme,
    focused_id: Option<&str>,
) {
    use ratatui::layout::{Constraint, Direction, Layout};

    // Fill background
    if style.bg.is_some() {
        let block = ratatui::widgets::Block::default().style(style);
        frame.render_widget(block, clip.viewport);
    }

    if children.is_empty() {
        return;
    }

    // Apply border and padding
    let (inner_area, block) = layout::apply_border(area, &node_layout.border, style);
    if let Some(block) = block {
        // Render border only within viewport
        let border_area = intersect_rects(area, clip.viewport);
        if border_area.width > 0 && border_area.height > 0 {
            frame.render_widget(block, border_area);
        }
    }
    let padded_area = layout::apply_padding(inner_area, node_layout.padding);

    // Calculate child layout
    let direction = if horizontal {
        Direction::Horizontal
    } else {
        Direction::Vertical
    };

    let constraints: Vec<Constraint> = children
        .iter()
        .enumerate()
        .flat_map(|(i, child)| {
            let mut v = vec![layout::child_constraint(child, horizontal)];
            if node_layout.gap > 0 && i < children.len() - 1 {
                v.push(Constraint::Length(node_layout.gap));
            }
            v
        })
        .collect();

    let chunks = Layout::default()
        .direction(direction)
        .constraints(constraints)
        .split(padded_area);

    // Render only visible children
    let mut chunk_idx = 0;
    for child in children {
        if chunk_idx >= chunks.len() {
            break;
        }

        let child_area = chunks[chunk_idx];
        
        // Translate child area by scroll offset for visibility check
        let virtual_y = child_area.y.saturating_sub(clip.offset_y);
        let virtual_area = ratatui::layout::Rect::new(
            child_area.x,
            virtual_y,
            child_area.width,
            child_area.height,
        );

        // Check if child is visible in viewport
        if rects_overlap(virtual_area, clip.viewport) {
            // Create adjusted clip for this child
            let child_clip = ClipRect {
                viewport: clip.viewport,
                offset_x: clip.offset_x,
                offset_y: clip.offset_y,
            };
            render_node_clipped(frame, child, child_area, &child_clip, hit_map, theme, focused_id);
        }

        chunk_idx += 1;
        // Skip gap chunks
        if node_layout.gap > 0 && chunk_idx < chunks.len() {
            chunk_idx += 1;
        }
    }
}

/// Check if two rectangles overlap
fn rects_overlap(a: ratatui::layout::Rect, b: ratatui::layout::Rect) -> bool {
    a.x < b.x + b.width
        && a.x + a.width > b.x
        && a.y < b.y + b.height
        && a.y + a.height > b.y
}

/// Intersect two rectangles
fn intersect_rects(a: ratatui::layout::Rect, b: ratatui::layout::Rect) -> ratatui::layout::Rect {
    let x = a.x.max(b.x);
    let y = a.y.max(b.y);
    let right = (a.x + a.width).min(b.x + b.width);
    let bottom = (a.y + a.height).min(b.y + b.height);
    
    if right > x && bottom > y {
        ratatui::layout::Rect::new(x, y, right - x, bottom - y)
    } else {
        ratatui::layout::Rect::new(0, 0, 0, 0)
    }
}
