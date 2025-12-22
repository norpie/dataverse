use ratatui::style::{Modifier, Style as RatatuiStyle};

use crate::styling::color::StyleColor;

/// Text and element styling
#[derive(Debug, Clone, Default)]
pub struct Style {
    /// Foreground color
    pub fg: Option<StyleColor>,
    /// Background color
    pub bg: Option<StyleColor>,
    /// Bold text
    pub bold: bool,
    /// Italic text
    pub italic: bool,
    /// Underlined text
    pub underline: bool,
    /// Dim/faint text
    pub dim: bool,
}

impl Style {
    /// Create a new empty style
    pub const fn new() -> Self {
        Self {
            fg: None,
            bg: None,
            bold: false,
            italic: false,
            underline: false,
            dim: false,
        }
    }

    /// Set foreground color
    pub fn fg(mut self, color: impl Into<StyleColor>) -> Self {
        self.fg = Some(color.into());
        self
    }

    /// Set background color
    pub fn bg(mut self, color: impl Into<StyleColor>) -> Self {
        self.bg = Some(color.into());
        self
    }

    /// Set bold
    pub const fn bold(mut self) -> Self {
        self.bold = true;
        self
    }

    /// Set italic
    pub const fn italic(mut self) -> Self {
        self.italic = true;
        self
    }

    /// Set underline
    pub const fn underline(mut self) -> Self {
        self.underline = true;
        self
    }

    /// Set dim
    pub const fn dim(mut self) -> Self {
        self.dim = true;
        self
    }

    /// Convert to ratatui style (only modifiers, colors require theme resolution).
    ///
    /// Note: This does not resolve colors - use `style_to_ratatui` in render.rs
    /// for full conversion with theme support.
    pub fn to_ratatui_modifiers(&self) -> RatatuiStyle {
        let mut style = RatatuiStyle::default();

        let mut modifiers = Modifier::empty();
        if self.bold {
            modifiers |= Modifier::BOLD;
        }
        if self.italic {
            modifiers |= Modifier::ITALIC;
        }
        if self.underline {
            modifiers |= Modifier::UNDERLINED;
        }
        if self.dim {
            modifiers |= Modifier::DIM;
        }

        if !modifiers.is_empty() {
            style = style.add_modifier(modifiers);
        }

        style
    }
}

impl From<Style> for RatatuiStyle {
    fn from(style: Style) -> Self {
        style.to_ratatui_modifiers()
    }
}
