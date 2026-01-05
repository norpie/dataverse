use std::time::Duration;

use tuidom::{Border, Color, Element, Edges, Size, Style};

/// Default duration for toast notifications.
pub const DEFAULT_TOAST_DURATION: Duration = Duration::from_secs(4);

/// Toast variant for different styles.
#[derive(Debug, Clone)]
pub enum ToastKind {
    Info,
    Success,
    Warning,
    Error,
}

/// A toast notification.
///
/// Toasts display temporary messages to the user. Use the convenience
/// constructors for common cases, or `custom()` for full control.
///
/// # Example
///
/// ```ignore
/// // Simple text toasts with default styling
/// cx.toast(Toast::info("File saved"));
/// cx.toast(Toast::error("Connection failed"));
/// ```
#[derive(Debug, Clone)]
pub struct Toast {
    /// The message to display.
    pub message: String,
    /// The toast kind for styling.
    pub kind: ToastKind,
    /// How long to show the toast.
    pub duration: Duration,
}

impl Toast {
    /// Create an info toast with neutral styling.
    pub fn info(title: impl Into<String>) -> Self {
        Self {
            message: title.into(),
            kind: ToastKind::Info,
            duration: DEFAULT_TOAST_DURATION,
        }
    }

    /// Create a success toast with green accent.
    pub fn success(title: impl Into<String>) -> Self {
        Self {
            message: title.into(),
            kind: ToastKind::Success,
            duration: DEFAULT_TOAST_DURATION,
        }
    }

    /// Create a warning toast with yellow accent.
    pub fn warning(title: impl Into<String>) -> Self {
        Self {
            message: title.into(),
            kind: ToastKind::Warning,
            duration: DEFAULT_TOAST_DURATION,
        }
    }

    /// Create an error toast with red accent.
    pub fn error(title: impl Into<String>) -> Self {
        Self {
            message: title.into(),
            kind: ToastKind::Error,
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
        let fg = match self.kind {
            ToastKind::Info => Color::oklch(0.8, 0.0, 0.0),
            ToastKind::Success => Color::oklch(0.7, 0.15, 145.0),
            ToastKind::Warning => Color::oklch(0.75, 0.15, 85.0),
            ToastKind::Error => Color::oklch(0.65, 0.2, 25.0),
        };

        Element::box_()
            .width(Size::Fill)
            .style(
                Style::new()
                    .foreground(fg)
                    .background(Color::oklch(0.2, 0.02, 250.0))
                    .border(Border::Rounded),
            )
            .padding(Edges::symmetric(1, 1))
            .child(Element::text(&self.message))
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
