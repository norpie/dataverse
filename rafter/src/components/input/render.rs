//! Input component rendering.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style as RatatuiStyle};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

/// Render an input field
pub fn render_input(
    frame: &mut Frame,
    value: &str,
    placeholder: &str,
    cursor: usize,
    style: RatatuiStyle,
    focused: bool,
    area: Rect,
) {
    let is_empty = value.is_empty();
    let display_text = if is_empty { placeholder } else { value };

    // Placeholder gets dimmed styling
    let text_style = if is_empty {
        style.add_modifier(Modifier::DIM)
    } else {
        style
    };

    if focused {
        // Build text with cursor at correct position
        // For placeholder, cursor is always at position 0
        let cursor_pos = if is_empty { 0 } else { cursor };

        // Find the character index for the cursor (handle UTF-8)
        let before_cursor: String = display_text
            .char_indices()
            .take_while(|(byte_idx, _)| *byte_idx < cursor_pos)
            .map(|(_, c)| c)
            .collect();

        let at_cursor: Option<char> = display_text[cursor_pos..]
            .chars()
            .next();

        let after_cursor: String = if let Some(c) = at_cursor {
            display_text[cursor_pos + c.len_utf8()..].to_string()
        } else {
            String::new()
        };

        // Cursor style: reverse video for visibility
        let cursor_style = text_style.add_modifier(Modifier::REVERSED);

        // Build spans
        let mut spans = vec![Span::styled(before_cursor, text_style)];

        if let Some(c) = at_cursor {
            // Cursor on existing character
            spans.push(Span::styled(c.to_string(), cursor_style));
            spans.push(Span::styled(after_cursor, text_style));
        } else {
            // Cursor at end - show a space as cursor
            spans.push(Span::styled(" ", cursor_style));
        }

        let line = Line::from(spans);
        let paragraph = Paragraph::new(line);
        frame.render_widget(paragraph, area);
    } else {
        // Not focused - simple render
        let paragraph = Paragraph::new(display_text).style(text_style);
        frame.render_widget(paragraph, area);
    }
}
