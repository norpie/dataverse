//! Spinner widget for loading states.
//!
//! Based on the snake spinner pattern - a bouncing gradient bar.
//! Uses the theme's accent color.

use std::time::Duration;

use rafter::{HandlerRegistry, WidgetHandlers};
use tuidom::{Color, Element, Style};

/// Configuration for the spinner.
#[derive(Clone, Debug)]
pub struct Spinner {
    /// Element ID for stable animation state.
    id: Option<String>,
    /// Width of the track in characters.
    track_width: u16,
    /// Length of the snake/bar.
    snake_len: u16,
    /// Pause frames at right end.
    right_pause: usize,
    /// Pause frames at left end.
    left_pause: usize,
    /// Frame duration in milliseconds.
    frame_ms: u64,
}

impl Default for Spinner {
    fn default() -> Self {
        Self {
            id: None,
            track_width: 8,
            snake_len: 6,
            right_pause: 1,
            left_pause: 20,
            frame_ms: 60,
        }
    }
}

impl Spinner {
    /// Create a new spinner with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the element ID for stable animation state.
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Set the track width.
    pub fn track_width(mut self, width: u16) -> Self {
        self.track_width = width;
        self
    }

    /// Set the snake/bar length.
    pub fn snake_len(mut self, len: u16) -> Self {
        self.snake_len = len;
        self
    }

    /// Set pause frames at right end.
    pub fn right_pause(mut self, frames: usize) -> Self {
        self.right_pause = frames;
        self
    }

    /// Set pause frames at left end.
    pub fn left_pause(mut self, frames: usize) -> Self {
        self.left_pause = frames;
        self
    }

    /// Set frame duration in milliseconds.
    pub fn frame_ms(mut self, ms: u64) -> Self {
        self.frame_ms = ms;
        self
    }

    /// Build the spinner element.
    ///
    /// Spinner is a stateless widget, so it doesn't use the registry or handlers,
    /// but accepts them for API consistency with other widgets.
    pub fn build(self, _registry: &HandlerRegistry, _handlers: &WidgetHandlers) -> Element {
        let mut elem = Element::frames(self.generate_frames(), Duration::from_millis(self.frame_ms));
        if let Some(id) = &self.id {
            elem = elem.id(id);
        }
        elem
    }

    fn generate_frames(&self) -> Vec<Element> {
        let mut frames = Vec::new();
        let track_width = self.track_width as i32;
        let snake_len = self.snake_len as i32;

        // Right pass: snake enters from left, travels across, exits right
        for head_pos in 0..=(track_width + snake_len - 2) {
            frames.push(self.make_snake_frame(head_pos, true));
        }

        // Pause after going right
        for _ in 0..self.right_pause {
            frames.push(self.make_empty_frame());
        }

        // Left pass: snake enters from right, travels across, exits left
        for head_pos in (0..=(track_width + snake_len - 2)).rev() {
            frames.push(self.make_snake_frame(head_pos, false));
        }

        // Pause after going left
        for _ in 0..self.left_pause {
            frames.push(self.make_empty_frame());
        }

        frames
    }

    fn make_empty_frame(&self) -> Element {
        let bg_color = Color::var("accent").darken(0.5);
        let mut row = Element::row();
        for _ in 0..self.track_width {
            row = row.child(Element::text("⬝").style(Style::new().foreground(bg_color.clone())));
        }
        row
    }

    fn make_snake_frame(&self, head_pos: i32, moving_right: bool) -> Element {
        let bg_color = Color::var("accent").darken(0.5);
        let track_width = self.track_width as i32;
        let snake_len = self.snake_len as i32;

        let mut row = Element::row();

        for i in 0..track_width {
            let snake_start = head_pos - snake_len + 1;

            let child = if i >= snake_start && i <= head_pos {
                let snake_idx = i - snake_start;

                let t = if moving_right {
                    snake_idx as f32 / (snake_len - 1) as f32
                } else {
                    1.0 - (snake_idx as f32 / (snake_len - 1) as f32)
                };

                // Interpolate color: tail is dim (t=0), head is bright (t=1)
                // darken by 0.4 at tail, 0.0 at head
                let darken_amount = 0.4 * (1.0 - t);
                Element::text("■")
                    .style(Style::new().foreground(Color::var("accent").darken(darken_amount)))
            } else {
                Element::text("⬝").style(Style::new().foreground(bg_color.clone()))
            };

            row = row.child(child);
        }

        row
    }
}
