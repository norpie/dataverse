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
//!     page! {
//!         text (color: primary) { "Hello" }
//!         text (color: text_muted) { "Muted" }
//!     }
//! }
//! ```

use std::sync::Arc;

use crate::color::{Color, StyleColor};

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
    /// Color for validation error text/messages
    pub validation_error: Color,
    /// Color for validation error widget border/highlight
    pub validation_error_border: Color,
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
            primary: Color::CYAN,
            secondary: Color::BLUE,
            background: Color::oklch(0.15, 0.0, 0.0), // Near black
            surface: Color::oklch(0.25, 0.0, 0.0),    // Dark gray
            text: Color::WHITE,
            text_muted: Color::GRAY,
            error: Color::RED,
            success: Color::GREEN,
            warning: Color::YELLOW,
            info: Color::CYAN,
            validation_error: Color::RED,
            validation_error_border: Color::RED,
        }
    }

    /// Create a light theme variant.
    pub fn light() -> Self {
        Self {
            primary: Color::BLUE,
            secondary: Color::CYAN,
            background: Color::WHITE,
            surface: Color::oklch(0.95, 0.0, 0.0), // Near white
            text: Color::BLACK,
            text_muted: Color::DARK_GRAY,
            error: Color::RED,
            success: Color::GREEN,
            warning: Color::YELLOW,
            info: Color::BLUE,
            validation_error: Color::RED,
            validation_error_border: Color::RED,
        }
    }
}

impl Theme for DefaultTheme {
    fn resolve(&self, name: &str) -> Option<Color> {
        match name {
            "primary" => Some(self.primary),
            "secondary" => Some(self.secondary),
            "background" => Some(self.background),
            "surface" => Some(self.surface),
            "text" => Some(self.text),
            "text_muted" => Some(self.text_muted),
            "error" => Some(self.error),
            "success" => Some(self.success),
            "warning" => Some(self.warning),
            "info" => Some(self.info),
            "validation_error" => Some(self.validation_error),
            "validation_error_border" => Some(self.validation_error_border),
            // Common aliases
            "fg" => Some(self.text),
            "bg" => Some(self.background),
            "muted" => Some(self.text_muted),
            "danger" => Some(self.error),
            // Basic color names
            "black" => Some(Color::BLACK),
            "red" => Some(Color::RED),
            "green" => Some(Color::GREEN),
            "yellow" => Some(Color::YELLOW),
            "blue" => Some(Color::BLUE),
            "magenta" => Some(Color::MAGENTA),
            "cyan" => Some(Color::CYAN),
            "white" => Some(Color::WHITE),
            "gray" | "grey" => Some(Color::GRAY),
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
            "validation_error",
            "validation_error_border",
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

/// Resolve a StyleColor to a concrete Color, looking up named colors in the theme.
///
/// This function is used by the renderer to convert `StyleColor` values
/// (which may contain named references) to concrete colors.
pub fn resolve_color(color: &StyleColor, theme: &dyn Theme) -> Color {
    match color {
        StyleColor::Concrete(c) => *c,
        StyleColor::Named(name) => theme.resolve(name).unwrap_or_else(|| {
            log::warn!("Unknown theme color '{}', using default", name);
            Color::GRAY
        }),
    }
}

/// Resolve a StyleColor to a concrete Color, returning None if the named color is not found.
pub fn resolve_style_color(color: &StyleColor, theme: &dyn Theme) -> Option<Color> {
    match color {
        StyleColor::Concrete(c) => Some(*c),
        StyleColor::Named(name) => theme.resolve(name),
    }
}
