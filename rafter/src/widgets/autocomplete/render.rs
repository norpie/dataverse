//! Rendering for the Autocomplete widget.

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

use super::Autocomplete;

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

/// Render the autocomplete trigger (text input with dropdown indicator).
pub fn render_trigger(
    frame: &mut Frame,
    area: Rect,
    autocomplete: &Autocomplete,
    focused: bool,
    ctx: &RenderContext<'_>,
) {
    let theme = ctx.theme;
    let value = autocomplete.value();
    let placeholder = autocomplete.placeholder();
    let is_empty = value.is_empty();
    let display_text = if is_empty { &placeholder } else { &value };

    // Focused state gets a subtle background highlight
    let base_style = if focused {
        RatatuiStyle::default()
            .fg(resolve_theme_color(theme, "text"))
            .bg(RatatuiColor::Rgb(80, 80, 100))
            .add_modifier(Modifier::BOLD)
    } else {
        RatatuiStyle::default().fg(resolve_theme_color(theme, "text"))
    };

    // Dimmed for placeholder
    let text_style = if is_empty {
        base_style.add_modifier(Modifier::DIM)
    } else {
        base_style
    };

    // Dropdown indicator
    let indicator = if autocomplete.is_open() { "▲" } else { "▼" };

    // Calculate available width for text (minus indicator + space)
    let inner_width = area.width.saturating_sub(2) as usize;

    // For focused input with value, show cursor
    let text_cursor = autocomplete.text_cursor();

    // Build spans for text with cursor
    let spans = if focused && !is_empty {
        // Show text with visible cursor position
        let before = &value[..text_cursor.min(value.len())];
        let cursor_char = value[text_cursor..].chars().next();
        let after_start = text_cursor + cursor_char.map(|c| c.len_utf8()).unwrap_or(0);
        let after = &value[after_start.min(value.len())..];

        let cursor_span = if let Some(c) = cursor_char {
            Span::styled(c.to_string(), text_style.add_modifier(Modifier::REVERSED))
        } else {
            // Cursor at end - show a block
            Span::styled(" ", text_style.add_modifier(Modifier::REVERSED))
        };

        // Truncate if necessary
        let max_before = inner_width.saturating_sub(1); // Leave room for cursor
        let truncated_before = if before.len() > max_before {
            &before[before.len() - max_before..]
        } else {
            before
        };

        let remaining = inner_width
            .saturating_sub(truncated_before.len())
            .saturating_sub(1);
        let truncated_after: String = after.chars().take(remaining).collect();
        let padding_len = inner_width
            .saturating_sub(truncated_before.len())
            .saturating_sub(1)
            .saturating_sub(truncated_after.len());

        vec![
            Span::styled(truncated_before.to_string(), text_style),
            cursor_span,
            Span::styled(truncated_after, text_style),
            Span::styled(" ".repeat(padding_len), base_style),
            Span::styled(" ", base_style),
            Span::styled(
                indicator,
                if focused {
                    RatatuiStyle::default()
                        .fg(resolve_theme_color(theme, "muted"))
                        .bg(RatatuiColor::Rgb(80, 80, 100))
                        .add_modifier(Modifier::DIM)
                } else {
                    RatatuiStyle::default()
                        .fg(resolve_theme_color(theme, "muted"))
                        .add_modifier(Modifier::DIM)
                },
            ),
        ]
    } else {
        // No cursor visible - just show text
        let truncated = if display_text.len() > inner_width {
            format!("{}…", &display_text[..inner_width.saturating_sub(1)])
        } else {
            format!("{:width$}", display_text, width = inner_width)
        };

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

        vec![
            Span::styled(truncated, text_style),
            Span::styled(" ", base_style),
            Span::styled(indicator, indicator_style),
        ]
    };

    let line = Line::from(spans);
    let paragraph = Paragraph::new(line);
    frame.render_widget(paragraph, area);

    // Render error message if present
    if let Some(error) = autocomplete.error() {
        render_error(frame, area, &error, theme);
    }
}

/// Render the error message below the autocomplete.
fn render_error(frame: &mut Frame, area: Rect, error: &str, theme: &dyn Theme) {
    let error_area = Rect {
        x: area.x,
        y: area.y + area.height,
        width: area.width,
        height: 1,
    };

    if error_area.y < frame.area().height {
        let error_text = Paragraph::new(error)
            .style(RatatuiStyle::default().fg(resolve_theme_color(theme, "error")));
        frame.render_widget(error_text, error_area);
    }
}

/// Build the dropdown content as a Node for the overlay.
pub fn build_dropdown_content(autocomplete: &Autocomplete, _ctx: &RenderContext<'_>) -> Node {
    let cursor = autocomplete.cursor();
    let filtered = autocomplete.filtered();

    // Build option rows from filtered items
    let option_nodes: Vec<Node> = filtered
        .iter()
        .enumerate()
        .filter_map(|(i, _filter_match)| {
            let label = autocomplete.filtered_label(i)?;
            let is_cursor = i == cursor;

            // Cursor: bright purple background
            // Normal: default colors
            let style = if is_cursor {
                let bg_color = Color::hex(0xA277FF);
                Style::new()
                    .bg(StyleColor::Concrete(bg_color))
                    .fg(StyleColor::Named("background".into()))
            } else {
                Style::new().fg(StyleColor::Named("text".to_string()))
            };

            let text_node = Node::Text {
                content: format!("{} ", label),
                style: Style::default(),
            };

            let row = Node::Row {
                children: vec![text_node],
                style,
                layout: Layout {
                    width: Size::Flex(1),
                    ..Default::default()
                },
                id: None,
            };

            Some(row)
        })
        .collect();

    // If no matches, show a "no matches" message
    let children = if option_nodes.is_empty() {
        vec![Node::Text {
            content: " No matches ".to_string(),
            style: Style::new().fg(StyleColor::Named("muted".to_string())),
        }]
    } else {
        let mut children = option_nodes;
        // Add bottom spacer
        children.push(Node::Text {
            content: " ".to_string(),
            style: Style::default(),
        });
        children
    };

    Node::Column {
        children,
        style: Style::default().bg(StyleColor::Named("surface".to_string())),
        layout: Layout {
            border: Border::None,
            padding: 0,
            gap: 0,
            ..Default::default()
        },
        id: None,
    }
}
