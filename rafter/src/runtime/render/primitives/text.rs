//! Text widget rendering.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style as RatatuiStyle;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

/// Render text content
pub fn render_text(frame: &mut Frame, content: &str, style: RatatuiStyle, area: Rect) {
    let span = Span::styled(content, style);
    let line = Line::from(span);
    let paragraph = Paragraph::new(line);
    frame.render_widget(paragraph, area);
}
