//! RadioGroup component rendering.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style as RatatuiStyle};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

/// Render a radio group (vertical list of radio options)
pub fn render_radio_group(
    frame: &mut Frame,
    options: &[String],
    selected: Option<usize>,
    selected_char: char,
    unselected_char: char,
    style: RatatuiStyle,
    focused: bool,
    focused_index: Option<usize>,
    area: Rect,
) {
    let lines: Vec<Line> = options
        .iter()
        .enumerate()
        .map(|(idx, label)| {
            let indicator = if selected == Some(idx) {
                selected_char
            } else {
                unselected_char
            };

            let content = format!("{} {}", indicator, label);

            // Highlight the focused option within the group
            let line_style = if focused && focused_index == Some(idx) {
                style
                    .bg(Color::Rgb(80, 80, 100))
                    .add_modifier(Modifier::BOLD)
            } else {
                style
            };

            Line::from(Span::styled(content, line_style))
        })
        .collect();

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, area);
}
