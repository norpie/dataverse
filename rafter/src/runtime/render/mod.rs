//! Rendering - convert Node tree to ratatui widgets.

mod backdrop;
pub(crate) mod layout;
mod model;
mod primitives;
pub(crate) mod system_overlay;
mod toasts;

pub use model::RenderNodeFn;

use std::collections::HashMap;

use ratatui::Frame;
use ratatui::style::Style as RatatuiStyle;

use super::animation::AnimationManager;
use super::hit_test::HitTestMap;
use crate::layers::overlay::OverlayRequest;
use crate::node::Node;
use crate::styling::style::Style;
use crate::styling::theme::{Theme, resolve_color};

pub use backdrop::{dim_backdrop, fill_background};
pub use toasts::{calculate_toast_removal_time, has_animating_toasts, render_toasts};

use crate::widgets::RenderContext;
use primitives::{render_container, render_stack, render_text};

use super::animation::{AnimatedProperty, AnimatedValue, Animation};

/// Detect style changes and start animations if transitions are enabled.
///
/// Called during widget rendering to check if animatable style properties changed.
fn check_style_transitions(
    widget_id: &str,
    new_style: &Style,
    previous_styles: &mut HashMap<String, Style>,
    animations: &mut AnimationManager,
) {
    // Get previous style if any
    let Some(prev_style) = previous_styles.get(widget_id) else {
        // First render - store style but don't animate
        log::debug!("First render for {}, storing style", widget_id);
        previous_styles.insert(widget_id.to_string(), new_style.clone());
        return;
    };

    // Check if transitions are enabled
    let Some(duration) = new_style.transition_duration else {
        // No transitions - just update stored style
        previous_styles.insert(widget_id.to_string(), new_style.clone());
        return;
    };

    let easing = new_style.transition_easing;

    // Check for background color lightness changes
    // Only animate if both old and new have concrete colors we can compare
    if let (Some(prev_bg), Some(new_bg)) = (&prev_style.bg, &new_style.bg) {
        // For now, we only animate if both are Concrete colors
        // Named colors would need theme resolution which we don't have here
        if let (
            crate::styling::StyleColor::Concrete(prev_color),
            crate::styling::StyleColor::Concrete(new_color),
        ) = (prev_bg, new_bg)
        {
            let prev_l = prev_color.lightness();
            let new_l = new_color.lightness();

            // Only animate if there's a meaningful difference
            if (prev_l - new_l).abs() > 0.001 {
                log::debug!(
                    "Starting bg lightness animation for {}: {} -> {} over {:?}",
                    widget_id, prev_l, new_l, duration
                );
                animations.start(Animation::transition(
                    widget_id,
                    AnimatedProperty::BackgroundLightness {
                        from: prev_l,
                        to: new_l,
                    },
                    duration,
                    easing,
                ));
            }
        }
    }

    // Check for foreground color lightness changes
    if let (Some(prev_fg), Some(new_fg)) = (&prev_style.fg, &new_style.fg) {
        if let (
            crate::styling::StyleColor::Concrete(prev_color),
            crate::styling::StyleColor::Concrete(new_color),
        ) = (prev_fg, new_fg)
        {
            let prev_l = prev_color.lightness();
            let new_l = new_color.lightness();

            if (prev_l - new_l).abs() > 0.001 {
                animations.start(Animation::transition(
                    widget_id,
                    AnimatedProperty::ForegroundLightness {
                        from: prev_l,
                        to: new_l,
                    },
                    duration,
                    easing,
                ));
            }
        }
    }

    // Check for opacity changes
    let prev_opacity = prev_style.opacity.unwrap_or(1.0);
    let new_opacity = new_style.opacity.unwrap_or(1.0);
    if (prev_opacity - new_opacity).abs() > 0.001 {
        animations.start(Animation::transition(
            widget_id,
            AnimatedProperty::Opacity {
                from: prev_opacity,
                to: new_opacity,
            },
            duration,
            easing,
        ));
    }

    // Update stored style
    previous_styles.insert(widget_id.to_string(), new_style.clone());
}

/// Convert a Style to ratatui Style (simple version without animations).
///
/// Used by clipped rendering paths that don't need animation support.
pub(crate) fn style_to_ratatui_simple(style: &Style, theme: &dyn Theme) -> RatatuiStyle {
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

/// Convert a Style to ratatui Style, resolving named colors via theme
/// and applying any active animation values.
pub(crate) fn style_to_ratatui(
    style: &Style,
    theme: &dyn Theme,
    widget_id: Option<&str>,
    animations: &AnimationManager,
) -> RatatuiStyle {
    let mut ratatui_style = RatatuiStyle::default();

    // Get animated values for this widget if any
    let animated_values = if let Some(id) = widget_id {
        animations.get_values(id)
    } else {
        Vec::new()
    };

    // Start with base colors and opacity
    let mut fg = style.fg.clone();
    let mut bg = style.bg.clone();
    let mut opacity = style.opacity;

    // Apply animated values (override base style)
    if !animated_values.is_empty() {
        log::debug!("Applying {} animated values for {:?}", animated_values.len(), widget_id);
    }
    for value in &animated_values {
        match value {
            AnimatedValue::BackgroundLightness(l) => {
                // Modify bg color lightness
                log::debug!("Applying animated bg lightness: {}", l);
                if let Some(ref color) = bg {
                    let resolved = resolve_color(color, theme);
                    bg = Some(crate::styling::StyleColor::Concrete(
                        resolved.with_lightness(*l),
                    ));
                }
            }
            AnimatedValue::ForegroundLightness(l) => {
                // Modify fg color lightness
                if let Some(ref color) = fg {
                    let resolved = resolve_color(color, theme);
                    fg = Some(crate::styling::StyleColor::Concrete(
                        resolved.with_lightness(*l),
                    ));
                }
            }
            AnimatedValue::Opacity(o) => {
                // Override static opacity with animated value
                opacity = Some(*o);
            }
        }
    }

    // Get the terminal background color for opacity blending
    // We use black as the base since terminals don't have true transparency
    let blend_base = crate::styling::color::Color::BLACK;

    // Apply resolved colors with opacity blending
    if let Some(ref fg_color) = fg {
        let mut resolved = resolve_color(fg_color, theme);
        if let Some(o) = opacity {
            // Blend toward base (simulate transparency by fading)
            resolved = resolved.blend(&blend_base, 1.0 - o);
        }
        ratatui_style = ratatui_style.fg(resolved.to_ratatui());
    }

    if let Some(ref bg_color) = bg {
        let mut resolved = resolve_color(bg_color, theme);
        if let Some(o) = opacity {
            // Blend toward base (simulate transparency by fading)
            resolved = resolved.blend(&blend_base, 1.0 - o);
        }
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
#[allow(clippy::too_many_arguments)]
pub fn render_node(
    frame: &mut Frame,
    node: &Node,
    area: ratatui::layout::Rect,
    hit_map: &mut HitTestMap,
    theme: &dyn Theme,
    focused_id: Option<&str>,
    overlay_requests: &mut Vec<OverlayRequest>,
    animations: &mut AnimationManager,
    previous_styles: &mut HashMap<String, Style>,
) {
    // Constrain area for auto-sized containers
    let area = layout::constrain_area(node, area);

    match node {
        Node::Empty => {}
        Node::Text { content, style } => {
            render_text(
                frame,
                content,
                style_to_ratatui(style, theme, None, animations),
                area,
            );
        }
        Node::Column {
            children,
            style,
            layout,
            id,
        } => {
            // Check for style transitions if this container has an ID
            if let Some(container_id) = id {
                check_style_transitions(container_id, style, previous_styles, animations);
            }
            render_container(
                frame,
                children,
                style_to_ratatui(style, theme, id.as_deref(), animations),
                layout,
                area,
                false,
                hit_map,
                theme,
                focused_id,
                overlay_requests,
                animations,
                previous_styles,
            );
        }
        Node::Row {
            children,
            style,
            layout,
            id,
        } => {
            // Check for style transitions if this container has an ID
            if let Some(container_id) = id {
                check_style_transitions(container_id, style, previous_styles, animations);
            }
            render_container(
                frame,
                children,
                style_to_ratatui(style, theme, id.as_deref(), animations),
                layout,
                area,
                true,
                hit_map,
                theme,
                focused_id,
                overlay_requests,
                animations,
                previous_styles,
            );
        }
        Node::Stack {
            children,
            style,
            layout,
            id,
        } => {
            // Check for style transitions if this container has an ID
            if let Some(container_id) = id {
                check_style_transitions(container_id, style, previous_styles, animations);
            }
            render_stack(
                frame,
                children,
                style_to_ratatui(style, theme, id.as_deref(), animations),
                layout,
                area,
                hit_map,
                theme,
                focused_id,
                overlay_requests,
                animations,
                previous_styles,
            );
        }
        Node::Widget {
            widget,
            style,
            layout,
            children,
            ..
        } => {
            // Check for style transitions and start animations if needed
            let widget_id = widget.id();
            check_style_transitions(&widget_id, style, previous_styles, animations);

            // Unified widget rendering - delegates to the widget's render method
            let is_focused = focused_id == Some(widget_id.as_str());
            let ratatui_style = style_to_ratatui(style, theme, Some(&widget_id), animations);
            let mut ctx = RenderContext {
                theme,
                hit_map,
                render_node,
                focused_id,
                style: ratatui_style,
                layout,
                children,
                overlay_requests,
                animations,
                previous_styles,
            };
            widget.render(frame, area, is_focused, &mut ctx);
            // Only register full area if widget doesn't handle its own hit registration
            if !widget.registers_own_hit_area() {
                ctx.hit_map
                    .register(widget.id(), area, widget.captures_input());
            }
        }
    }
}
