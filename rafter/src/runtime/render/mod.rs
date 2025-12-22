//! Rendering - convert Node tree to ratatui widgets.

mod backdrop;
pub(crate) mod layout;
mod model;
mod primitives;
mod toasts;

pub use model::RenderNodeFn;

use ratatui::Frame;
use ratatui::style::Style as RatatuiStyle;

use super::hit_test::HitTestMap;
use crate::node::Node;
use crate::overlay::OverlayRequest;
use crate::style::Style;
use crate::theme::{Theme, resolve_color};

pub use backdrop::{dim_backdrop, fill_background};
pub use toasts::render_toasts;

use crate::widgets::RenderContext;
use primitives::{render_container, render_stack, render_text};

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
    overlay_requests: &mut Vec<OverlayRequest>,
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
                overlay_requests,
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
                overlay_requests,
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
                overlay_requests,
            );
        }
        Node::Widget {
            widget,
            style,
            layout,
            children,
            ..
        } => {
            // Unified widget rendering - delegates to the widget's render method
            let is_focused = focused_id == Some(widget.id().as_str());
            let ratatui_style = style_to_ratatui(style, theme);
            let mut ctx = RenderContext {
                theme,
                hit_map,
                render_node,
                focused_id,
                style: ratatui_style,
                layout,
                children,
                overlay_requests,
            };
            widget.render(frame, area, is_focused, &mut ctx);
            // Only register full area if widget doesn't handle its own hit registration
            if !widget.registers_own_hit_area() {
                ctx.hit_map.register(widget.id(), area, widget.captures_input());
            }
        }
    }
}
