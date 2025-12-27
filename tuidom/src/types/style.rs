use super::{Border, Color, TextStyle};

#[derive(Debug, Clone, Default)]
pub struct Style {
    pub background: Option<Color>,
    pub foreground: Option<Color>,
    pub border: Border,
    pub text_style: TextStyle,
}

impl Style {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn background(mut self, color: Color) -> Self {
        self.background = Some(color);
        self
    }

    pub fn foreground(mut self, color: Color) -> Self {
        self.foreground = Some(color);
        self
    }

    pub fn border(mut self, border: Border) -> Self {
        self.border = border;
        self
    }

    pub fn text_style(mut self, text_style: TextStyle) -> Self {
        self.text_style = text_style;
        self
    }

    pub fn bold(mut self) -> Self {
        self.text_style.bold = true;
        self
    }

    pub fn italic(mut self) -> Self {
        self.text_style.italic = true;
        self
    }

    pub fn underline(mut self) -> Self {
        self.text_style.underline = true;
        self
    }

    pub fn dim(mut self) -> Self {
        self.text_style.dim = true;
        self
    }
}
