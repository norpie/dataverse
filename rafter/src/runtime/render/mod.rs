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
use crate::style::Style;
use crate::theme::{Theme, resolve_color};

pub use backdrop::{dim_backdrop, fill_background};
pub use toasts::render_toasts;

use crate::widgets::input::render::render_input;
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
#[allow(deprecated)] // Allow legacy Node variants during migration
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
        Node::Widget {
            widget,
            style: _,
            layout: _,
            ..
        } => {
            // Unified widget rendering - delegates to the widget's render method
            let is_focused = focused_id == Some(widget.id().as_str());
            widget.render(frame, area, is_focused);
            hit_map.register(widget.id(), area, widget.captures_input());
        }
        // Legacy variants (deprecated - to be removed in Phase 5)
        Node::Input {
            value,
            placeholder,
            style,
            id,
            widget,
            ..
        } => {
            let is_focused = focused_id == Some(id.as_str());
            let (display_value, cursor_pos) = widget
                .as_ref()
                .map(|c| (c.value(), c.cursor()))
                .unwrap_or_else(|| (value.clone(), value.len()));
            render_input(
                frame,
                &display_value,
                placeholder,
                cursor_pos,
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
        Node::Checkbox {
            id,
            style,
            widget,
            ..
        } => {
            let is_focused = focused_id == Some(id.as_str());
            crate::widgets::checkbox::render::render_checkbox(
                frame,
                widget.is_checked(),
                &widget.label(),
                widget.checked_char(),
                widget.unchecked_char(),
                style_to_ratatui(style, theme),
                is_focused,
                area,
            );
            if !id.is_empty() {
                hit_map.register(id.clone(), area, false);
            }
        }
        Node::RadioGroup {
            id,
            style,
            widget,
            ..
        } => {
            let is_focused = focused_id == Some(id.as_str());
            // For radio group, focused_index is the currently selected option for keyboard nav
            // When focused, highlight the selected option (or first if none selected)
            let focused_index = if is_focused {
                Some(widget.selected().unwrap_or(0))
            } else {
                None
            };
            crate::widgets::radio::render::render_radio_group(
                frame,
                &widget.options(),
                widget.selected(),
                widget.selected_char(),
                widget.unselected_char(),
                style_to_ratatui(style, theme),
                is_focused,
                focused_index,
                area,
            );
            if !id.is_empty() {
                hit_map.register(id.clone(), area, false);
            }
        }
        Node::ScrollArea {
            child,
            id,
            style,
            widget,
            ..
        } => {
            crate::widgets::scroll_area::render::render(
                frame,
                child,
                id,
                style_to_ratatui(style, theme),
                widget,
                area,
                hit_map,
                theme,
                focused_id,
                style_to_ratatui,
                render_node,
            );
        }
        Node::List {
            id,
            style,
            layout,
            widget,
            ..
        } => {
            crate::widgets::list::render::render(
                frame,
                id,
                style_to_ratatui(style, theme),
                layout,
                widget.as_ref(),
                area,
                hit_map,
                theme,
                focused_id,
                render_node,
            );
        }
        Node::Tree {
            id,
            style,
            layout,
            widget,
            ..
        } => {
            crate::widgets::tree::render::render(
                frame,
                id,
                style_to_ratatui(style, theme),
                layout,
                widget.as_ref(),
                area,
                hit_map,
                theme,
                focused_id,
                render_node,
            );
        }
        Node::Table {
            id,
            style,
            layout,
            widget,
            ..
        } => {
            crate::widgets::table::render::render(
                frame,
                id,
                style_to_ratatui(style, theme),
                layout,
                widget.as_ref(),
                area,
                hit_map,
                theme,
            );
        }
    }
}
