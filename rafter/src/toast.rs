use std::time::Duration;

use tuidom::{Border, Color, Edges, Element, Size, Style};

/// Default duration for toast notifications.
pub const DEFAULT_TOAST_DURATION: Duration = Duration::from_secs(4);

/// A toast notification.
///
/// Toasts display temporary messages to the user. Use the convenience
/// constructors for common cases, or `custom()` for full control.
///
/// # Example
///
/// ```ignore
/// // Simple text toasts with default styling
/// gx.toast(Toast::info("File saved"));
/// gx.toast(Toast::error("Connection failed"));
///
/// // Custom element
/// gx.toast(Toast::custom(
///     Element::row()
///         .child(Element::text("Processing..."))
///         .child(spinner())
/// ));
/// ```
#[derive(Debug, Clone)]
pub struct Toast {
    /// The content to display.
    pub content: Element,
    /// How long to show the toast.
    pub duration: Duration,
}

impl Toast {
    /// Create an info toast with neutral styling.
    pub fn info(message: impl Into<String>) -> Self {
        Self {
            content: Element::text(message.into())
                .style(Style::new().foreground(Color::oklch(0.8, 0.0, 0.0))),
            duration: DEFAULT_TOAST_DURATION,
        }
    }

    /// Create a success toast with green accent.
    pub fn success(message: impl Into<String>) -> Self {
        Self {
            content: Element::text(message.into())
                .style(Style::new().foreground(Color::oklch(0.7, 0.15, 145.0))),
            duration: DEFAULT_TOAST_DURATION,
        }
    }

    /// Create a warning toast with yellow accent.
    pub fn warning(message: impl Into<String>) -> Self {
        Self {
            content: Element::text(message.into())
                .style(Style::new().foreground(Color::oklch(0.75, 0.15, 85.0))),
            duration: DEFAULT_TOAST_DURATION,
        }
    }

    /// Create an error toast with red accent.
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            content: Element::text(message.into())
                .style(Style::new().foreground(Color::oklch(0.65, 0.2, 25.0))),
            duration: DEFAULT_TOAST_DURATION,
        }
    }

    /// Create a toast with custom content.
    ///
    /// Use this when you need full control over the toast's appearance.
    pub fn custom(content: Element) -> Self {
        Self {
            content,
            duration: DEFAULT_TOAST_DURATION,
        }
    }

    /// Set a custom duration for this toast.
    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }

    /// Build the toast's element for rendering.
    pub fn element(&self) -> Element {
        Element::box_()
            .width(Size::Fill)
            .style(
                Style::new()
                    .background(Color::oklch(0.2, 0.02, 250.0))
                    .border(Border::Rounded),
            )
            .padding(Edges::symmetric(1, 1))
            .child(self.content.clone())
    }
}

impl From<String> for Toast {
    fn from(message: String) -> Self {
        Toast::info(message)
    }
}

impl From<&str> for Toast {
    fn from(message: &str) -> Self {
        Toast::info(message)
    }
}
