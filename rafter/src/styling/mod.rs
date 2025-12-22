//! Styling module - colors, styles, and themes.

pub mod color;
pub mod style;
pub mod theme;

pub use color::{Color, NamedColor, StyleColor};
pub use style::Style;
pub use theme::{DefaultTheme, Theme, ThemeRef, resolve_color, resolve_style_color};
