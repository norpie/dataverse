#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Size {
    Fixed(u16),
    #[default]
    Fill,
    Flex(u16),
    Auto,
    Percent(f32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Direction {
    Row,
    #[default]
    Column,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Position {
    #[default]
    Static,
    Relative,
    Absolute,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Overflow {
    #[default]
    Visible,
    Hidden,
    Scroll,
    Auto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Justify {
    #[default]
    Start,
    Center,
    End,
    SpaceBetween,
    SpaceAround,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Align {
    #[default]
    Start,
    Center,
    End,
    Stretch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Wrap {
    #[default]
    NoWrap,
    Wrap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextWrap {
    #[default]
    NoWrap,
    WordWrap,
    CharWrap,
    Truncate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Border {
    #[default]
    None,
    Single,
    Double,
    Rounded,
    Thick,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TextStyle {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub dim: bool,
    pub strikethrough: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum Backdrop {
    #[default]
    None,
    /// Darken the entire buffer by amount (0.0-1.0)
    Dim(f32),
    /// Reduce chroma of the entire buffer by amount (0.0-1.0)
    Desaturate(f32),
}

impl TextStyle {
    pub const fn new() -> Self {
        Self {
            bold: false,
            italic: false,
            underline: false,
            dim: false,
            strikethrough: false,
        }
    }

    pub const fn bold(mut self) -> Self {
        self.bold = true;
        self
    }

    pub const fn italic(mut self) -> Self {
        self.italic = true;
        self
    }

    pub const fn underline(mut self) -> Self {
        self.underline = true;
        self
    }

    pub const fn dim(mut self) -> Self {
        self.dim = true;
        self
    }

    pub const fn strikethrough(mut self) -> Self {
        self.strikethrough = true;
        self
    }

    /// Merge another text style on top of this one.
    /// Boolean fields use OR logic (if either is true, result is true).
    pub fn merge(&self, other: &TextStyle) -> TextStyle {
        TextStyle {
            bold: other.bold || self.bold,
            italic: other.italic || self.italic,
            underline: other.underline || self.underline,
            dim: other.dim || self.dim,
            strikethrough: other.strikethrough || self.strikethrough,
        }
    }
}
