use crate::types::{Rgb, TextStyle};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    pub char: char,
    pub fg: Rgb,
    pub bg: Rgb,
    pub style: TextStyle,
    pub wide_continuation: bool,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            char: ' ',
            fg: Rgb::new(255, 255, 255),
            bg: Rgb::new(0, 0, 0),
            style: TextStyle::new(),
            wide_continuation: false,
        }
    }
}

impl Cell {
    pub fn new(char: char) -> Self {
        Self {
            char,
            ..Default::default()
        }
    }

    pub fn with_fg(mut self, fg: Rgb) -> Self {
        self.fg = fg;
        self
    }

    pub fn with_bg(mut self, bg: Rgb) -> Self {
        self.bg = bg;
        self
    }

    pub fn with_style(mut self, style: TextStyle) -> Self {
        self.style = style;
        self
    }
}
