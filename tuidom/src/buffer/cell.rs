use crate::types::{Oklch, TextStyle};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cell {
    pub char: char,
    pub fg: Oklch,
    /// Background color. `None` means transparent (use terminal default).
    pub bg: Option<Oklch>,
    pub style: TextStyle,
    pub wide_continuation: bool,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            char: ' ',
            fg: Oklch::new(1.0, 0.0, 0.0), // white
            bg: None,                       // transparent
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

    pub fn with_bg(mut self, bg: Option<Oklch>) -> Self {
        self.bg = bg;
        self
    }

    pub fn with_style(mut self, style: TextStyle) -> Self {
        self.style = style;
        self
    }
}
