//! Single-character braille spinner for inline loading indicators.
//!
//! Displays a spinning animation using braille dot patterns.
//! Perfect for compact loading states in lists and tables.

use std::time::Duration;

use rafter::{HandlerRegistry, WidgetHandlers};
use tuidom::Element;

/// Simple single-character braille spinner.
///
/// Cycles through braille patterns to create a spinning animation.
/// Takes exactly 1 character of space, making it perfect for inline indicators.
///
/// # Example
///
/// ```ignore
/// page! {
///     row (gap: 1) {
///         braille_spinner (id: "loading")
///         text (content: "Loading data...")
///     }
/// }
/// ```
#[derive(Clone, Debug)]
pub struct BrailleSpinner {
    /// Element ID for stable animation state.
    id: Option<String>,
    /// Frame duration in milliseconds.
    frame_ms: u64,
}

impl Default for BrailleSpinner {
    fn default() -> Self {
        Self {
            id: None,
            frame_ms: 80,
        }
    }
}

impl BrailleSpinner {
    /// Create a new braille spinner with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the element ID for stable animation state.
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Set the frame duration in milliseconds (default: 80ms).
    pub fn frame_ms(mut self, ms: u64) -> Self {
        self.frame_ms = ms;
        self
    }

    /// Build the spinner element.
    ///
    /// This is a stateless widget, so it doesn't use registry or handlers,
    /// but accepts them for API consistency with other widgets.
    pub fn build(self, _registry: &HandlerRegistry, _handlers: &WidgetHandlers) -> Element {
        self.build_standalone()
    }

    /// Build the spinner element without registry/handlers.
    ///
    /// Use this when building the spinner outside of the page! macro context.
    pub fn build_standalone(self) -> Element {
        // Classic braille spinner pattern (clockwise rotation)
        let frames = vec![
            Element::text("⠋"), // dots 1,2,4,6
            Element::text("⠙"), // dots 2,4,5,6
            Element::text("⠹"), // dots 1,2,4,5,6
            Element::text("⠸"), // dots 4,5,6
            Element::text("⠼"), // dots 3,4,5,6
            Element::text("⠴"), // dots 3,5,6
            Element::text("⠦"), // dots 2,3,6
            Element::text("⠧"), // dots 1,2,3,6
            Element::text("⠇"), // dots 1,2,3
            Element::text("⠏"), // dots 1,2,3,4
        ];

        let mut elem = Element::frames(frames, Duration::from_millis(self.frame_ms));
        if let Some(id) = &self.id {
            elem = elem.id(id);
        }
        elem
    }
}
