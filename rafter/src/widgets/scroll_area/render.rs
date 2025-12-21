//! ScrollArea widget rendering.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style as RatatuiStyle;

use super::ScrollArea;
use super::ScrollbarConfig;
use super::ScrollbarVisibility;
use super::state::ScrollDirection;
use crate::widgets::scrollbar::ScrollbarState;
use crate::node::Node;
use crate::runtime::hit_test::HitTestMap;
use crate::runtime::render::RenderNodeFn;
use crate::style::Style;
use crate::theme::Theme;
use crate::utils::geometry::{intersect_rects, rects_overlap};
use crate::utils::text::wrap_text;

// Re-export from shared scrollbar module
pub use crate::widgets::scrollbar::{render_horizontal_scrollbar, render_vertical_scrollbar};

// Re-export ClipRect for convenience
pub use crate::utils::geometry::ClipRect;

/// Function type for converting Style to RatatuiStyle.
pub type StyleToRatatuiFn = fn(&Style, &dyn Theme) -> RatatuiStyle;

/// Render a scroll area container.
#[allow(clippy::too_many_arguments)]
pub fn render(
    frame: &mut Frame,
    child: &Node,
    id: &str,
    style: RatatuiStyle,
    widget: &ScrollArea,
    area: Rect,
    hit_map: &mut HitTestMap,
    theme: &dyn Theme,
    focused_id: Option<&str>,
    style_to_ratatui: StyleToRatatuiFn,
    render_node: RenderNodeFn,
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
        widget.direction(),
        &widget.scrollbar_config(),
    );

    // Calculate actual content height with wrapping based on viewport width
    let content_size = calculate_wrapped_content_size(child, scroll_layout.content_area.width);

    // Update widget with computed sizes
    widget.set_sizes(
        content_size,
        (
            scroll_layout.content_area.width,
            scroll_layout.content_area.height,
        ),
    );

    // Get scroll offset
    let (offset_x, offset_y) = widget.offset();

    // Render scrollbars and save geometry for hit testing
    let v_geom = if scroll_layout.show_vertical {
        render_vertical_scrollbar(
            frame.buffer_mut(),
            area,
            offset_y,
            content_size.1,
            scroll_layout.content_area.height,
            &widget.scrollbar_config(),
            theme,
        )
    } else {
        None
    };
    widget.set_vertical_scrollbar(v_geom);

    let h_geom = if scroll_layout.show_horizontal {
        render_horizontal_scrollbar(
            frame.buffer_mut(),
            area,
            offset_x,
            content_size.0,
            scroll_layout.content_area.width,
            &widget.scrollbar_config(),
            theme,
        )
    } else {
        None
    };
    widget.set_horizontal_scrollbar(h_geom);

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

/// Render state for a scroll area, computed during rendering.
pub struct ScrollAreaRenderState {
    /// Area for the content (excluding scrollbars).
    pub content_area: Rect,
    /// Whether to show vertical scrollbar.
    pub show_vertical: bool,
    /// Whether to show horizontal scrollbar.
    pub show_horizontal: bool,
}

/// Calculate the layout and determine scrollbar visibility.
pub fn calculate_scroll_area_layout(
    area: Rect,
    content_size: (u16, u16),
    direction: ScrollDirection,
    config: &ScrollbarConfig,
) -> ScrollAreaRenderState {
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
        ScrollbarVisibility::Always => {
            matches!(direction, ScrollDirection::Vertical | ScrollDirection::Both)
        }
        ScrollbarVisibility::Never => false,
        ScrollbarVisibility::Auto => needs_vertical,
    };

    let show_horizontal = match config.horizontal {
        ScrollbarVisibility::Always => {
            matches!(
                direction,
                ScrollDirection::Horizontal | ScrollDirection::Both
            )
        }
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

    ScrollAreaRenderState {
        content_area,
        show_vertical,
        show_horizontal,
    }
}

/// Render a node with viewport clipping (for scroll area content).
#[allow(clippy::too_many_arguments)]
pub fn render_node_clipped(
    frame: &mut Frame,
    node: &Node,
    area: Rect,
    clip: &ClipRect,
    hit_map: &mut HitTestMap,
    theme: &dyn Theme,
    focused_id: Option<&str>,
    style_to_ratatui: fn(&crate::style::Style, &dyn Theme) -> RatatuiStyle,
    render_node: fn(&mut Frame, &Node, Rect, &mut HitTestMap, &dyn Theme, Option<&str>),
) {
    use crate::runtime::render::layout;

    match node {
        Node::Empty => {}
        Node::Text { content, style } => {
            render_text_clipped(frame, content, style_to_ratatui(style, theme), clip);
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
                style_to_ratatui,
                render_node,
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
                style_to_ratatui,
                render_node,
            );
        }
        Node::Stack {
            children,
            style,
            layout: node_layout,
        } => {
            let ratatui_style = style_to_ratatui(style, theme);
            if ratatui_style.bg.is_some() {
                let block = ratatui::widgets::Block::default().style(ratatui_style);
                frame.render_widget(block, area);
            }
            let (inner_area, block) =
                layout::apply_border(area, &node_layout.border, ratatui_style);
            if let Some(block) = block {
                frame.render_widget(block, area);
            }
            let padded_area = layout::apply_padding(inner_area, node_layout.padding);
            for child in children {
                render_node_clipped(
                    frame,
                    child,
                    padded_area,
                    clip,
                    hit_map,
                    theme,
                    focused_id,
                    style_to_ratatui,
                    render_node,
                );
            }
        }
        // For other node types, fall back to regular rendering
        _ => {
            render_node(frame, node, area, hit_map, theme, focused_id);
        }
    }
}

/// Render text with vertical clipping (skip lines above viewport, stop at bottom).
fn render_text_clipped(frame: &mut Frame, content: &str, style: RatatuiStyle, clip: &ClipRect) {
    use ratatui::widgets::{Paragraph, Wrap};

    let viewport_width = clip.viewport.width as usize;
    let wrapped_lines = wrap_text(content, viewport_width);
    let total_lines = wrapped_lines.len();

    let start_line = clip.offset_y as usize;
    let visible_lines = clip.viewport.height as usize;
    let end_line = (start_line + visible_lines).min(total_lines);

    if start_line >= total_lines {
        return;
    }

    let visible_text = wrapped_lines[start_line..end_line].join("\n");

    let paragraph = Paragraph::new(visible_text)
        .style(style)
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, clip.viewport);
}

/// Render a container with clipping.
#[allow(clippy::too_many_arguments)]
fn render_container_clipped(
    frame: &mut Frame,
    children: &[Node],
    style: RatatuiStyle,
    node_layout: &crate::node::Layout,
    area: Rect,
    horizontal: bool,
    clip: &ClipRect,
    hit_map: &mut HitTestMap,
    theme: &dyn Theme,
    focused_id: Option<&str>,
    style_to_ratatui: fn(&crate::style::Style, &dyn Theme) -> RatatuiStyle,
    render_node: fn(&mut Frame, &Node, Rect, &mut HitTestMap, &dyn Theme, Option<&str>),
) {
    use crate::runtime::render::layout;
    use ratatui::layout::{Constraint, Direction, Layout};

    if style.bg.is_some() {
        let block = ratatui::widgets::Block::default().style(style);
        frame.render_widget(block, clip.viewport);
    }

    if children.is_empty() {
        return;
    }

    let (inner_area, block) = layout::apply_border(area, &node_layout.border, style);
    if let Some(block) = block {
        let border_area = intersect_rects(area, clip.viewport);
        if border_area.width > 0 && border_area.height > 0 {
            frame.render_widget(block, border_area);
        }
    }
    let padded_area = layout::apply_padding(inner_area, node_layout.padding);

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

    let mut chunk_idx = 0;
    for child in children {
        if chunk_idx >= chunks.len() {
            break;
        }

        let child_area = chunks[chunk_idx];

        let virtual_y = child_area.y.saturating_sub(clip.offset_y);
        let virtual_area = Rect::new(child_area.x, virtual_y, child_area.width, child_area.height);

        if rects_overlap(virtual_area, clip.viewport) {
            let child_clip = ClipRect {
                viewport: clip.viewport,
                offset_x: clip.offset_x,
                offset_y: clip.offset_y,
            };
            render_node_clipped(
                frame,
                child,
                child_area,
                &child_clip,
                hit_map,
                theme,
                focused_id,
                style_to_ratatui,
                render_node,
            );
        }

        chunk_idx += 1;
        if node_layout.gap > 0 && chunk_idx < chunks.len() {
            chunk_idx += 1;
        }
    }
}

/// Calculate content size with text wrapping taken into account.
pub fn calculate_wrapped_content_size(node: &Node, viewport_width: u16) -> (u16, u16) {
    match node {
        Node::Text { content, .. } => {
            let wrapped = wrap_text(content, viewport_width as usize);
            (viewport_width, wrapped.len() as u16)
        }
        Node::Column {
            children, layout, ..
        } => {
            let border_size = if matches!(layout.border, crate::node::Border::None) {
                0
            } else {
                2
            };
            let padding = layout.padding * 2;
            let inner_width = viewport_width.saturating_sub(padding + border_size);

            let child_heights: u16 = children
                .iter()
                .map(|c| calculate_wrapped_content_size(c, inner_width).1)
                .sum();
            let gaps = if children.len() > 1 {
                layout.gap * (children.len() as u16 - 1)
            } else {
                0
            };
            (viewport_width, child_heights + gaps + padding + border_size)
        }
        Node::Row {
            children, layout, ..
        } => {
            let border_size = if matches!(layout.border, crate::node::Border::None) {
                0
            } else {
                2
            };
            let padding = layout.padding * 2;

            let child_count = children.len().max(1) as u16;
            let gaps = if children.len() > 1 {
                layout.gap * (children.len() as u16 - 1)
            } else {
                0
            };
            let available = viewport_width.saturating_sub(padding + border_size + gaps);
            let child_width = available / child_count;

            let max_height = children
                .iter()
                .map(|c| calculate_wrapped_content_size(c, child_width).1)
                .max()
                .unwrap_or(0);
            (viewport_width, max_height + padding + border_size)
        }
        _ => (node.intrinsic_width(), node.intrinsic_height()),
    }
}
