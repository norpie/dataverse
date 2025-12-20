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
use crate::theme::{resolve_color, Theme};

pub use backdrop::{dim_backdrop, fill_background};
pub use toasts::render_toasts;

use crate::components::input::render::render_input;
use crate::components::scrollable::render::{
    calculate_scrollable_layout, calculate_wrapped_content_size, render_horizontal_scrollbar,
    render_node_clipped, render_vertical_scrollbar, ClipRect,
};
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
            let display_value = component
                .as_ref()
                .map(|c| c.value())
                .unwrap_or_else(|| value.clone());
            render_input(
                frame,
                &display_value,
                placeholder,
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
        Node::Scrollable {
            child,
            id,
            style,
            component,
            ..
        } => {
            render_scrollable(
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
    }
}

/// Render a scrollable container
#[allow(clippy::too_many_arguments)]
fn render_scrollable(
    frame: &mut Frame,
    child: &Node,
    id: &str,
    style: RatatuiStyle,
    component: &crate::components::Scrollable,
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
    let scroll_layout = calculate_scrollable_layout(
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

    // Register hit box for scroll area
    if !id.is_empty() {
        hit_map.register(id.to_string(), area, false);
    }
}
