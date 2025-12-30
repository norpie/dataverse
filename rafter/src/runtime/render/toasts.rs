//! Toast notification rendering.

use std::time::{Duration, Instant};

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style as RatatuiStyle};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph, Wrap};

use crate::context::{Toast, ToastLevel};
use crate::styling::theme::Theme;

/// Duration for slide animations (in/out/up)
const SLIDE_DURATION: Duration = Duration::from_millis(150);

/// Duration for slide-out animation (exported for retention calculation)
pub const SLIDE_OUT_DURATION: Duration = SLIDE_DURATION;

/// Maximum number of visible toasts
const MAX_VISIBLE_TOASTS: usize = 5;

/// Toast dimensions
const TOAST_WIDTH: u16 = 44;
const TOAST_MARGIN: u16 = 1;
const TOP_OFFSET: u16 = 1;

/// Easing function (ease-out cubic)
fn ease_out(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}

/// Easing function (ease-in cubic)
fn ease_in(t: f32) -> f32 {
    t.powi(3)
}

/// Calculate when a toast at a given index becomes "active" (starts its visible lifetime).
/// This is when it slides in, and its display duration starts counting from here.
fn calculate_activation_time(toast_index: usize, toasts: &[(Toast, Instant)]) -> Instant {
    if toast_index == 0 {
        // First toast activates immediately when created
        return toasts[0].1;
    }

    if toast_index < MAX_VISIBLE_TOASTS {
        // Within initial visible set - activates when created
        return toasts[toast_index].1;
    }

    // This toast needs to wait for a slot to open up.
    // It activates when the toast that's blocking it expires.
    // The blocking toast is (toast_index - MAX_VISIBLE_TOASTS) positions before us.
    let blocking_index = toast_index - MAX_VISIBLE_TOASTS;
    let blocking_activation = calculate_activation_time(blocking_index, toasts);
    let blocking_toast = &toasts[blocking_index].0;

    // We activate when the blocking toast's visible lifetime ends
    blocking_activation + blocking_toast.duration
}

/// Render active toasts in the top-right corner with animations.
///
/// Animation behavior:
/// - Toasts slide in from the right when they become visible
/// - Toasts slide up when toasts above them expire
/// - Toasts slide out to the right when they expire
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

    // Track current Y position in pixels (not slots, since heights vary)
    let mut current_y: f32 = TOP_OFFSET as f32;

    for (i, (toast, _created_at)) in toasts.iter().enumerate() {
        // Calculate when this toast becomes active (visible lifetime starts)
        let activation_time = calculate_activation_time(i, toasts);

        // Has this toast been activated yet?
        if now < activation_time {
            // Still waiting in queue, don't render
            continue;
        }

        let time_since_activation = now.duration_since(activation_time);

        // The toast's visible lifetime is its duration, starting from activation
        let expiry_time = activation_time + toast.duration;
        let time_until_expiry = expiry_time.saturating_duration_since(now);
        let time_since_expiry = now.saturating_duration_since(expiry_time);

        // Calculate slide-in progress (0.0 = just activated, 1.0 = fully visible)
        let slide_in_progress = if time_since_activation >= SLIDE_DURATION {
            1.0
        } else {
            ease_out(time_since_activation.as_secs_f32() / SLIDE_DURATION.as_secs_f32())
        };

        // Calculate slide-out progress (0.0 = still visible, 1.0 = fully slid out)
        let slide_out_progress = if time_until_expiry >= SLIDE_DURATION {
            0.0
        } else if time_since_expiry > Duration::ZERO {
            // Already past expiry, continue slide-out animation
            ease_in(
                (time_since_expiry.as_secs_f32() / SLIDE_DURATION.as_secs_f32()).min(1.0)
                    + (1.0 - time_until_expiry.as_secs_f32() / SLIDE_DURATION.as_secs_f32())
                        .max(0.0),
            )
            .min(1.0)
        } else {
            ease_in(1.0 - time_until_expiry.as_secs_f32() / SLIDE_DURATION.as_secs_f32())
        };

        // If fully slid out, skip rendering
        if slide_out_progress >= 1.0 {
            continue;
        }

        // Calculate toast height
        let toast_height = toast.height(TOAST_WIDTH);

        // Calculate horizontal offset
        let slide_in_x = (1.0 - slide_in_progress) * (TOAST_WIDTH as f32 + 2.0);
        let slide_out_x = slide_out_progress * (TOAST_WIDTH as f32 + 2.0);
        let total_x_offset = (slide_in_x + slide_out_x) as u16;

        // Calculate Y position
        let toast_y = current_y as u16;

        // Update Y position for next toast
        // A sliding-out toast takes less space, allowing others to slide up
        let effective_height =
            (toast_height as f32 + TOAST_MARGIN as f32) * (1.0 - slide_out_progress);
        current_y += effective_height;

        // Position toast
        let base_x = area.width.saturating_sub(TOAST_WIDTH + 1);
        let toast_x = base_x.saturating_add(total_x_offset).min(area.width);
        let visible_width = area.width.saturating_sub(toast_x);

        // Skip if off-screen
        if visible_width == 0 || toast_y >= area.height {
            continue;
        }

        let toast_area = Rect::new(
            toast_x,
            toast_y,
            visible_width.min(TOAST_WIDTH),
            toast_height.min(area.height - toast_y),
        );

        // Get accent color from theme based on toast level
        let theme_color_name = match toast.level {
            ToastLevel::Info => "info",
            ToastLevel::Success => "success",
            ToastLevel::Warning => "warning",
            ToastLevel::Error => "error",
        };

        let accent_color = theme
            .resolve(theme_color_name)
            .map(|c| c.to_ratatui())
            .unwrap_or(Color::White);

        let bg_color = Color::Rgb(35, 35, 45);
        let text_color = Color::Rgb(220, 220, 230);
        let muted_color = Color::Rgb(160, 160, 170);

        // Clear the area first (so toasts appear on top)
        frame.render_widget(Clear, toast_area);

        // Build the toast content
        let mut lines: Vec<Line> = Vec::new();

        // Title line with icon - truncate if needed
        let max_title_width = (toast_area.width as usize).saturating_sub(4);
        let title = if toast.title.len() > max_title_width {
            format!("{}...", &toast.title[..max_title_width.saturating_sub(3)])
        } else {
            toast.title.clone()
        };

        lines.push(Line::from(vec![
            Span::styled(" ● ", RatatuiStyle::default().fg(accent_color)),
            Span::styled(
                title,
                RatatuiStyle::default()
                    .fg(text_color)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));

        // Body lines (if present)
        if let Some(body) = &toast.body {
            for line in body.lines() {
                lines.push(Line::from(vec![
                    Span::styled(
                        "   ", // Indent to align with title
                        RatatuiStyle::default(),
                    ),
                    Span::styled(line.to_string(), RatatuiStyle::default().fg(muted_color)),
                ]));
            }
        }

        // Use a Block to fill the background, then render the paragraph inside
        let block = Block::default().style(RatatuiStyle::default().bg(bg_color));
        let paragraph = Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .block(block);
        frame.render_widget(paragraph, toast_area);
    }
}

/// Calculate the removal time for a toast at a given index.
/// This accounts for queued toasts that haven't activated yet.
pub fn calculate_toast_removal_time(toast_index: usize, toasts: &[(Toast, Instant)]) -> Instant {
    let activation_time = calculate_activation_time(toast_index, toasts);
    let toast = &toasts[toast_index].0;
    activation_time + toast.duration + SLIDE_OUT_DURATION
}

/// Check if any toasts are currently animating (for triggering re-renders)
pub fn has_animating_toasts(toasts: &[(Toast, Instant)], now: Instant) -> bool {
    toasts.iter().enumerate().any(|(i, (toast, _created_at))| {
        let activation_time = calculate_activation_time(i, toasts);

        // Not yet activated - will animate when it does
        if now < activation_time {
            return false; // Not animating yet, but will be
        }

        let time_since_activation = now.duration_since(activation_time);
        let expiry_time = activation_time + toast.duration;
        let time_until_expiry = expiry_time.saturating_duration_since(now);
        let time_since_expiry = now.saturating_duration_since(expiry_time);

        // Animating if sliding in, sliding out, or in slide-out grace period
        time_since_activation < SLIDE_DURATION
            || time_until_expiry < SLIDE_DURATION
            || time_since_expiry < SLIDE_DURATION
    })
}
