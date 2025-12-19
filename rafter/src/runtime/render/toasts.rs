//! Toast notification rendering.

use std::time::Instant;

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style as RatatuiStyle};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};

use crate::context::{Toast, ToastLevel};
use crate::theme::Theme;

/// Render active toasts in the bottom-right corner
pub fn render_toasts(frame: &mut Frame, toasts: &[(Toast, Instant)], theme: &dyn Theme) {
    if toasts.is_empty() {
        return;
    }

    let area = frame.area();

    // Calculate toast dimensions - sleek single-line toasts
    const TOAST_WIDTH: u16 = 44;
    const TOAST_HEIGHT: u16 = 1;
    const TOAST_MARGIN: u16 = 1;

    // Render toasts from bottom to top
    for (i, (toast, _expiry)) in toasts.iter().enumerate().take(5) {
        let y_offset = (i as u16) * (TOAST_HEIGHT + TOAST_MARGIN);

        // Position in bottom-right corner
        let toast_area = Rect::new(
            area.width.saturating_sub(TOAST_WIDTH + 1),
            area.height.saturating_sub(TOAST_HEIGHT + 1 + y_offset),
            TOAST_WIDTH,
            TOAST_HEIGHT,
        );

        // Skip if toast would be off-screen
        if toast_area.y == 0 || toast_area.x == 0 {
            continue;
        }

        // Get accent color and icon from theme based on toast level
        let (theme_color_name, icon) = match toast.level {
            ToastLevel::Info => ("info", "●"),
            ToastLevel::Success => ("success", "✓"),
            ToastLevel::Warning => ("warning", "⚠"),
            ToastLevel::Error => ("error", "✗"),
        };

        let accent_color = theme
            .resolve(theme_color_name)
            .map(|c| c.to_ratatui())
            .unwrap_or(Color::White);

        let bg_color = Color::Rgb(35, 35, 45);

        // Clear the area first (so toasts appear on top)
        frame.render_widget(Clear, toast_area);

        // Render message with icon - truncate if needed
        let max_msg_width = (toast_area.width as usize).saturating_sub(4); // icon + spaces
        let message = if toast.message.len() > max_msg_width {
            format!("{}...", &toast.message[..max_msg_width.saturating_sub(3)])
        } else {
            toast.message.clone()
        };

        let line = Line::from(vec![
            Span::styled(
                format!(" {} ", icon),
                RatatuiStyle::default().fg(accent_color).bg(bg_color),
            ),
            Span::styled(
                format!("{} ", message),
                RatatuiStyle::default().fg(Color::Rgb(220, 220, 230)).bg(bg_color),
            ),
        ]);

        let paragraph = Paragraph::new(line);
        frame.render_widget(paragraph, toast_area);
    }
}
