//! Theme system for Rafter
//!
//! Themes define named colors that can be referenced in views using `color: name` syntax.
//! At render time, named colors are resolved by looking up the current theme.
//!
//! # Example
//!
//! ```rust,ignore
//! use rafter::prelude::*;
//!
//! // Define a custom theme (once #[theme] macro is implemented)
//! #[theme]
//! struct MyTheme {
//!     primary: Color,
//!     secondary: Color,
//!     background: Color,
//!     surface: Color,
//!     text: Color,
//!     text_muted: Color,
//!     error: Color,
//!     success: Color,
//!     warning: Color,
//! }
//!
//! impl MyTheme {
//!     fn dark() -> Self {
//!         Self {
//!             primary: Color::oklch(0.6, 0.15, 250.0),
//!             secondary: Color::oklch(0.7, 0.1, 200.0),
//!             background: Color::oklch(0.15, 0.02, 250.0),
//!             surface: Color::oklch(0.2, 0.02, 250.0),
//!             text: Color::oklch(0.9, 0.02, 250.0),
//!             text_muted: Color::oklch(0.6, 0.02, 250.0),
//!             error: Color::oklch(0.6, 0.2, 25.0),
//!             success: Color::oklch(0.6, 0.15, 145.0),
//!             warning: Color::oklch(0.7, 0.15, 85.0),
//!         }
//!     }
//! }
//!
//! // Use in views - named colors resolve via theme
//! fn my_view() -> Node {
//!     view! {
//!         text (color: primary) { "Hello" }
//!         text (color: text_muted) { "Muted" }
//!     }
//! }
//! ```

use std::sync::Arc;

use crate::color::Color;

/// Trait for theme types that can resolve named colors.
///
/// Implement this trait to create custom themes. The `#[theme]` macro
/// automatically implements this trait for annotated structs.
pub trait Theme: Send + Sync + 'static {
    /// Resolve a named color to its actual color value.
    ///
    /// Returns `None` if the color name is not defined in this theme.
    fn resolve(&self, name: &str) -> Option<Color>;

    /// Get all color names defined in this theme.
    fn color_names(&self) -> Vec<&'static str>;

    /// Clone this theme into a boxed trait object.
    fn clone_box(&self) -> Box<dyn Theme>;
}

impl Clone for Box<dyn Theme> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

/// A thread-safe reference to a theme.
pub type ThemeRef = Arc<dyn Theme>;

/// The default theme with standard color definitions.
///
/// This theme provides a dark color scheme suitable for terminal applications.
#[derive(Debug, Clone)]
pub struct DefaultTheme {
    pub primary: Color,
    pub secondary: Color,
    pub background: Color,
    pub surface: Color,
    pub text: Color,
    pub text_muted: Color,
    pub error: Color,
    pub success: Color,
    pub warning: Color,
    pub info: Color,
}

impl Default for DefaultTheme {
    fn default() -> Self {
        Self::dark()
    }
}

impl DefaultTheme {
    /// Create the default dark theme.
    pub fn dark() -> Self {
        Self {
            primary: Color::Cyan,
            secondary: Color::Blue,
            background: Color::Reset,
            surface: Color::Indexed(236), // Dark gray
            text: Color::White,
            text_muted: Color::Indexed(245), // Light gray
            error: Color::Red,
            success: Color::Green,
            warning: Color::Yellow,
            info: Color::Cyan,
        }
    }

    /// Create a light theme variant.
    pub fn light() -> Self {
        Self {
            primary: Color::Blue,
            secondary: Color::Cyan,
            background: Color::White,
            surface: Color::Indexed(255), // Near white
            text: Color::Black,
            text_muted: Color::Indexed(240), // Dark gray
            error: Color::Red,
            success: Color::Green,
            warning: Color::Yellow,
            info: Color::Blue,
        }
    }
}

impl Theme for DefaultTheme {
    fn resolve(&self, name: &str) -> Option<Color> {
        match name {
            "primary" => Some(self.primary.clone()),
            "secondary" => Some(self.secondary.clone()),
            "background" => Some(self.background.clone()),
            "surface" => Some(self.surface.clone()),
            "text" => Some(self.text.clone()),
            "text_muted" => Some(self.text_muted.clone()),
            "error" => Some(self.error.clone()),
            "success" => Some(self.success.clone()),
            "warning" => Some(self.warning.clone()),
            "info" => Some(self.info.clone()),
            // Common aliases
            "fg" => Some(self.text.clone()),
            "bg" => Some(self.background.clone()),
            "muted" => Some(self.text_muted.clone()),
            "danger" => Some(self.error.clone()),
            // Basic color names pass through
            "black" => Some(Color::Black),
            "red" => Some(Color::Red),
            "green" => Some(Color::Green),
            "yellow" => Some(Color::Yellow),
            "blue" => Some(Color::Blue),
            "magenta" => Some(Color::Magenta),
            "cyan" => Some(Color::Cyan),
            "white" => Some(Color::White),
            "gray" | "grey" => Some(Color::Indexed(245)),
            _ => None,
        }
    }

    fn color_names(&self) -> Vec<&'static str> {
        vec![
            "primary",
            "secondary",
            "background",
            "surface",
            "text",
            "text_muted",
            "error",
            "success",
            "warning",
            "info",
            "fg",
            "bg",
            "muted",
            "danger",
            "black",
            "red",
            "green",
            "yellow",
            "blue",
            "magenta",
            "cyan",
            "white",
            "gray",
            "grey",
        ]
    }

    fn clone_box(&self) -> Box<dyn Theme> {
        Box::new(self.clone())
    }
}

/// Resolve a color, looking up named colors in the theme.
///
/// This function is used by the renderer to convert `Color` values
/// (which may contain named references) to concrete colors.
pub fn resolve_color(color: &Color, theme: &dyn Theme) -> Color {
    match color {
        Color::Named(name) => theme.resolve(name).unwrap_or_else(|| {
            log::warn!("Unknown theme color '{}', using default", name);
            Color::Reset
        }),
        other => other.clone(),
    }
}
