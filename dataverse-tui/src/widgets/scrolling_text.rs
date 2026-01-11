//! Scrolling text widget - news ticker style animation.
//!
//! Displays text that scrolls horizontally when it overflows the available width.
//! If text fits, renders as a simple static text element.

use std::time::Duration;

use tuidom::text::{char_width, display_width};
use tuidom::{Element, Style};

use rafter::{HandlerRegistry, WidgetHandlers};

/// A scrolling text widget (news ticker style).
///
/// When text fits within the specified width, renders as static text.
/// When text overflows, animates by scrolling left, pausing at edges.
///
/// # Example
///
/// ```ignore
/// // In page! macro:
/// scrolling_text (text: "Very long text that will scroll...", width: 20, id: "ticker")
/// ```
#[derive(Clone, Debug)]
pub struct ScrollingText {
    text: String,
    width: u16,
    id: Option<String>,
    speed_ms: u64,
    pause_start_ms: u64,
    pause_end_ms: u64,
    style: Option<Style>,
}

impl Default for ScrollingText {
    fn default() -> Self {
        Self {
            text: String::new(),
            width: 20,
            id: None,
            speed_ms: 150,
            pause_start_ms: 1000,
            pause_end_ms: 1000,
            style: None,
        }
    }
}

impl ScrollingText {
    /// Create a new scrolling text widget.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the text content.
    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.text = text.into();
        self
    }

    /// Set the available width in characters.
    pub fn width(mut self, width: u16) -> Self {
        self.width = width;
        self
    }

    /// Set the element ID for stable animation state.
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Set the scroll speed in milliseconds per character.
    ///
    /// Default: 150ms
    pub fn speed(mut self, ms: u64) -> Self {
        self.speed_ms = ms;
        self
    }

    /// Set the pause duration at the start in milliseconds.
    ///
    /// Default: 1000ms
    pub fn pause_start(mut self, ms: u64) -> Self {
        self.pause_start_ms = ms;
        self
    }

    /// Set the pause duration at the end in milliseconds.
    ///
    /// Default: 1000ms
    pub fn pause_end(mut self, ms: u64) -> Self {
        self.pause_end_ms = ms;
        self
    }

    /// Set the text style.
    pub fn style(mut self, style: Style) -> Self {
        self.style = Some(style);
        self
    }

    /// Build the scrolling text element.
    pub fn build(self, _registry: &HandlerRegistry, _handlers: &WidgetHandlers) -> Element {
        let text_width = display_width(&self.text);
        let available_width = self.width as usize;

        // If text fits, return simple text element
        if text_width <= available_width {
            let mut elem = Element::text(&self.text);
            if let Some(id) = &self.id {
                elem = elem.id(id);
            }
            if let Some(style) = self.style {
                elem = elem.style(style);
            }
            return elem;
        }

        // Text overflows - create scrolling animation
        let frames = self.generate_frames(text_width, available_width);
        let mut elem = Element::frames(frames, Duration::from_millis(self.speed_ms));

        if let Some(id) = &self.id {
            elem = elem.id(id);
        }
        if let Some(style) = self.style {
            elem = elem.style(style);
        }

        elem
    }

    fn generate_frames(&self, text_width: usize, available_width: usize) -> Vec<Element> {
        let mut frames = Vec::new();

        // Calculate how many scroll positions we need
        // We scroll until the end of text is visible
        let max_offset = text_width.saturating_sub(available_width);

        // Pause frames at start
        let pause_start_frames = (self.pause_start_ms / self.speed_ms).max(1) as usize;
        let start_text = slice_by_display_width(&self.text, 0, available_width);
        for _ in 0..pause_start_frames {
            frames.push(Element::text(&start_text));
        }

        // Scroll frames (skip offset 0, we already showed it in pause)
        for offset in 1..=max_offset {
            let visible_text = slice_by_display_width(&self.text, offset, available_width);
            frames.push(Element::text(&visible_text));
        }

        // Pause frames at end
        let pause_end_frames = (self.pause_end_ms / self.speed_ms).max(1) as usize;
        let end_text = slice_by_display_width(&self.text, max_offset, available_width);
        for _ in 0..pause_end_frames {
            frames.push(Element::text(&end_text));
        }

        frames
    }
}

/// Extract a substring by display width offset and max width.
///
/// This handles Unicode characters with varying display widths correctly.
fn slice_by_display_width(s: &str, offset: usize, max_width: usize) -> String {
    let mut result = String::new();
    let mut current_offset = 0;
    let mut result_width = 0;

    for ch in s.chars() {
        let ch_width = char_width(ch);

        // Skip characters until we reach the offset
        if current_offset < offset {
            current_offset += ch_width;
            continue;
        }

        // Stop if adding this char would exceed max_width
        if result_width + ch_width > max_width {
            break;
        }

        result.push(ch);
        result_width += ch_width;
    }

    // Pad with spaces if we didn't fill the width
    // (can happen at end of text)
    while result_width < max_width {
        result.push(' ');
        result_width += 1;
    }

    result
}
