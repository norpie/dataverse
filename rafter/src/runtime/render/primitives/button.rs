//! Button widget rendering.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style as RatatuiStyle};
use ratatui::widgets::Paragraph;

/// Render a button
pub fn render_button(
    frame: &mut Frame,
    label: &str,
    style: RatatuiStyle,
    focused: bool,
    area: Rect,
) {
    // Buttons have a subtle background, brighter when focused
    let button_style = if focused {
        style
            .bg(Color::Rgb(80, 80, 100))
            .add_modifier(ratatui::style::Modifier::BOLD)
    } else {
        style.bg(Color::Rgb(50, 50, 65))
    };

    // Add padding with spaces
    let content = format!(" {} ", label);
    let paragraph = Paragraph::new(content).style(button_style);
    frame.render_widget(paragraph, area);
}
