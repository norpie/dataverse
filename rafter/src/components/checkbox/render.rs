//! Checkbox component rendering.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style as RatatuiStyle};
use ratatui::widgets::Paragraph;

/// Render a checkbox
pub fn render_checkbox(
    frame: &mut Frame,
    checked: bool,
    label: &str,
    checked_char: char,
    unchecked_char: char,
    style: RatatuiStyle,
    focused: bool,
    area: Rect,
) {
    let indicator = if checked {
        checked_char
    } else {
        unchecked_char
    };

    let content = if label.is_empty() {
        indicator.to_string()
    } else {
        format!("{} {}", indicator, label)
    };

    // Focused state gets a subtle background highlight
    let checkbox_style = if focused {
        style
            .bg(Color::Rgb(80, 80, 100))
            .add_modifier(ratatui::style::Modifier::BOLD)
    } else {
        style
    };

    let paragraph = Paragraph::new(content).style(checkbox_style);
    frame.render_widget(paragraph, area);
}
