//! Input component rendering.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style as RatatuiStyle;
use ratatui::widgets::Paragraph;

/// Render an input field
pub fn render_input(
    frame: &mut Frame,
    value: &str,
    placeholder: &str,
    style: RatatuiStyle,
    focused: bool,
    area: Rect,
) {
    let display_text = if value.is_empty() { placeholder } else { value };

    let mut input_style = style;
    if focused {
        input_style = input_style.add_modifier(ratatui::style::Modifier::REVERSED);
    }

    // Show cursor at end if focused
    let content = if focused {
        format!("{}█", display_text)
    } else {
        display_text.to_string()
    };

    let paragraph = Paragraph::new(content).style(input_style);
    frame.render_widget(paragraph, area);
}
