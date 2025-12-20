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
    widget.set_sizes(content_size, (scroll_layout.content_area.width, scroll_layout.content_area.height));

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

    // TODO: Clip and offset child rendering
    // For now, just render the child in the content area
    // This needs proper viewport clipping implementation
    render_node(
        frame,
        child,
        scroll_layout.content_area,
        hit_map,
        theme,
        focused_id,
    );

    // Register hit box for scroll area
    if !id.is_empty() {
        hit_map.register(id.to_string(), area, false);
    }
}
