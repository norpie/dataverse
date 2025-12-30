use crate::types::{Oklch, TextStyle};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cell {
    pub char: char,
    pub fg: Oklch,
    pub bg: Oklch,
    pub style: TextStyle,
    pub wide_continuation: bool,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            char: ' ',
            fg: Oklch::new(1.0, 0.0, 0.0), // white
            bg: Oklch::new(0.0, 0.0, 0.0), // black
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

    pub fn with_fg(mut self, fg: Oklch) -> Self {
        self.fg = fg;
        self
    }

    pub fn with_bg(mut self, bg: Oklch) -> Self {
        self.bg = bg;
        self
    }

    pub fn with_style(mut self, style: TextStyle) -> Self {
        self.style = style;
        self
    }
}
