//! Toast notification rendering.

use std::time::{Duration, Instant};

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style as RatatuiStyle};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};

use crate::context::{Toast, ToastLevel};
use crate::styling::theme::Theme;

/// Duration for slide-in animation
const SLIDE_IN_DURATION: Duration = Duration::from_millis(150);

/// Duration for slide-out animation
pub const SLIDE_OUT_DURATION: Duration = Duration::from_millis(150);

/// Easing function for slide-in animation (ease-out cubic)
fn ease_out_cubic(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}

/// Easing function for slide-out animation (ease-in cubic)
fn ease_in_cubic(t: f32) -> f32 {
    t.powi(3)
}

/// Render active toasts in the top-right corner with slide animations
pub fn render_toasts(
    frame: &mut Frame,
    toasts: &[(Toast, Instant)],
    theme: &dyn Theme,
    now: Instant,
) {
    if toasts.is_empty() {
        return;
    }

    let area = frame.area();

    // Calculate toast dimensions - sleek single-line toasts
    const TOAST_WIDTH: u16 = 44;
    const TOAST_HEIGHT: u16 = 1;
    const TOAST_MARGIN: u16 = 1;
    const TOP_OFFSET: u16 = 1; // Distance from top edge

    // Render toasts from top to bottom (newest first)
    for (i, (toast, created_at)) in toasts.iter().enumerate().take(5) {
        let age = now.duration_since(*created_at);
        let expiry = *created_at + toast.duration;
        let time_until_expiry = expiry.saturating_duration_since(now);

        // Calculate slide-in progress (0.0 = just created, 1.0 = fully visible)
        let slide_in_progress = if age >= SLIDE_IN_DURATION {
            1.0
        } else {
            ease_out_cubic(age.as_secs_f32() / SLIDE_IN_DURATION.as_secs_f32())
        };

        // Calculate slide-out progress (0.0 = still visible, 1.0 = fully slid out)
        let slide_out_progress = if time_until_expiry >= SLIDE_OUT_DURATION {
            0.0
        } else {
            ease_in_cubic(1.0 - time_until_expiry.as_secs_f32() / SLIDE_OUT_DURATION.as_secs_f32())
        };

        // Calculate horizontal offset for slide-in (starts off-screen to the right)
        let slide_in_x_offset = ((1.0 - slide_in_progress) * (TOAST_WIDTH as f32 + 2.0)) as u16;

        // Calculate horizontal offset for slide-out (slides back off-screen to the right)
        let slide_out_x_offset = (slide_out_progress * (TOAST_WIDTH as f32 + 2.0)) as u16;

        // Combined horizontal offset
        let total_x_offset = slide_in_x_offset + slide_out_x_offset;

        let y_offset = (i as u16) * (TOAST_HEIGHT + TOAST_MARGIN);

        // Position in top-right corner with slide animations
        let base_x = area.width.saturating_sub(TOAST_WIDTH + 1);
        let toast_x = base_x.saturating_add(total_x_offset).min(area.width);
        let visible_width = area.width.saturating_sub(toast_x);

        // Skip if toast would be completely off-screen horizontally
        if visible_width == 0 {
            continue;
        }

        let toast_area = Rect::new(
            toast_x,
            TOP_OFFSET + y_offset,
            visible_width.min(TOAST_WIDTH),
            TOAST_HEIGHT,
        );

        // Skip if toast would be off-screen vertically
        if toast_area.y >= area.height {
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
                RatatuiStyle::default()
                    .fg(Color::Rgb(220, 220, 230))
                    .bg(bg_color),
            ),
        ]);

        let paragraph = Paragraph::new(line);
        frame.render_widget(paragraph, toast_area);
    }
}

/// Check if any toasts are currently animating (for triggering re-renders)
pub fn has_animating_toasts(toasts: &[(Toast, Instant)], now: Instant) -> bool {
    toasts.iter().any(|(toast, created_at)| {
        let age = now.duration_since(*created_at);
        let expiry = *created_at + toast.duration;
        let time_until_expiry = expiry.saturating_duration_since(now);

        // Animating if sliding in OR sliding out
        age < SLIDE_IN_DURATION || time_until_expiry < SLIDE_OUT_DURATION
    })
}
