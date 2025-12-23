//! Rendering for the Select widget.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color as RatatuiColor, Modifier, Style as RatatuiStyle};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::node::{Border, Layout, Node, Size};
use crate::styling::color::{Color, StyleColor};
use crate::styling::style::Style;
use crate::styling::theme::Theme;
use crate::widgets::traits::RenderContext;

use super::Select;

/// Convert our Color to ratatui Color
fn to_ratatui_color(color: Color) -> RatatuiColor {
    let (r, g, b) = color.to_rgb();
    RatatuiColor::Rgb(r, g, b)
}

/// Resolve a theme color name to ratatui Color
fn resolve_theme_color(theme: &dyn Theme, name: &str) -> RatatuiColor {
    theme
        .resolve(name)
        .map(to_ratatui_color)
        .unwrap_or(RatatuiColor::Gray)
}

/// Render the select trigger (the inline closed appearance, matching Input style).
pub fn render_trigger(
    frame: &mut Frame,
    area: Rect,
    select: &Select,
    focused: bool,
    ctx: &RenderContext<'_>,
) {
    let theme = ctx.theme;

    // Build the display text
    let display_text = select
        .selected_label()
        .unwrap_or_else(|| select.placeholder());

    let is_placeholder = select.selected_index().is_none();

    // Focused state gets a subtle background highlight (same as checkbox/collapsible/radio)
    let base_style = if focused {
        RatatuiStyle::default()
            .fg(resolve_theme_color(theme, "text"))
            .bg(RatatuiColor::Rgb(80, 80, 100))
            .add_modifier(Modifier::BOLD)
    } else {
        RatatuiStyle::default().fg(resolve_theme_color(theme, "text"))
    };

    // Match Input widget styling: dimmed for placeholder, normal for value
    let text_style = if is_placeholder {
        base_style.add_modifier(Modifier::DIM)
    } else {
        base_style
    };

    // Create the dropdown indicator
    let indicator = if select.is_open() { "▲" } else { "▼" };

    // Calculate available width for text (minus indicator + space)
    let inner_width = area.width.saturating_sub(2) as usize; // 2 for indicator + space
    let truncated_text = if display_text.len() > inner_width {
        format!("{}…", &display_text[..inner_width.saturating_sub(1)])
    } else {
        display_text
    };

    // Build the line with text and indicator
    // Indicator style: inherit focus background if focused
    let indicator_style = if focused {
        RatatuiStyle::default()
            .fg(resolve_theme_color(theme, "muted"))
            .bg(RatatuiColor::Rgb(80, 80, 100))
            .add_modifier(Modifier::DIM)
    } else {
        RatatuiStyle::default()
            .fg(resolve_theme_color(theme, "muted"))
            .add_modifier(Modifier::DIM)
    };

    let line = Line::from(vec![
        Span::styled(
            format!("{:width$}", truncated_text, width = inner_width),
            text_style,
        ),
        Span::styled(" ", base_style), // Space inherits focus background
        Span::styled(indicator, indicator_style),
    ]);

    let paragraph = Paragraph::new(line);
    frame.render_widget(paragraph, area);

    // Render error message if present
    if let Some(error) = select.error() {
        render_error(frame, area, &error, theme);
    }
}

/// Render the error message below the select.
fn render_error(frame: &mut Frame, area: Rect, error: &str, theme: &dyn Theme) {
    let error_area = Rect {
        x: area.x,
        y: area.y + area.height,
        width: area.width,
        height: 1,
    };

    // Only render if there's space
    if error_area.y < frame.area().height {
        let error_text = Paragraph::new(error)
            .style(RatatuiStyle::default().fg(resolve_theme_color(theme, "error")));
        frame.render_widget(error_text, error_area);
    }
}

/// Build the dropdown content as a Node for the overlay.
pub fn build_dropdown_content(select: &Select, ctx: &RenderContext<'_>) -> Node {
    let cursor = select.cursor();

    // Build option rows from children (text nodes)
    let option_nodes: Vec<Node> = ctx
        .children
        .iter()
        .enumerate()
        .filter_map(|(i, child)| {
            if let Node::Text { content, .. } = child {
                let is_cursor = i == cursor;
                let is_selected = select.selected_index() == Some(i);

                // Match List widget theming:
                // - Cursor: bright purple background, inverted foreground
                // - Selected: dimmer purple background, inverted foreground
                // - Normal: default colors
                let style = if is_cursor {
                    let bg_color = Color::hex(0xA277FF); // Bright purple for cursor
                    Style::new()
                        .bg(StyleColor::Concrete(bg_color))
                        .fg(StyleColor::Named("background".into()))
                } else if is_selected {
                    let bg_color = Color::hex(0x6E5494); // Dimmer purple for selected
                    Style::new()
                        .bg(StyleColor::Concrete(bg_color))
                        .fg(StyleColor::Named("background".into()))
                } else {
                    Style::new().fg(StyleColor::Named("text".to_string()))
                };

                // Create a row with flex width to fill the dropdown
                // Add right padding (1 space) to the text, no left padding to align with trigger
                let text_node = Node::Text {
                    content: format!("{} ", content),
                    style: Style::default(),
                };

                let row = Node::Row {
                    children: vec![text_node],
                    style,
                    layout: Layout {
                        width: Size::Flex(1),
                        ..Default::default()
                    },
                };

                Some(row)
            } else {
                None
            }
        })
        .collect();

    // Add a bottom spacer for bottom padding effect
    let mut children = option_nodes;
    children.push(Node::Text {
        content: " ".to_string(),
        style: Style::default(),
    });

    // Wrap in a column without border (padding is handled inline)
    Node::Column {
        children,
        style: Style::default().bg(StyleColor::Named("surface".to_string())),
        layout: Layout {
            border: Border::None,
            padding: 0,
            gap: 0,
            ..Default::default()
        },
    }
}
