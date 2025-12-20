//! Layout types and properties for nodes.

/// Layout direction
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Direction {
    /// Vertical layout (column)
    #[default]
    Vertical,
    /// Horizontal layout (row)
    Horizontal,
}

/// Content alignment on the main axis
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Justify {
    #[default]
    Start,
    Center,
    End,
    SpaceBetween,
    SpaceAround,
}

/// Content alignment on the cross axis
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Align {
    Start,
    Center,
    End,
    #[default]
    Stretch,
}

/// Border style
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Border {
    #[default]
    None,
    Single,
    Double,
    Rounded,
    Thick,
}

/// Size specification
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Size {
    /// Fixed size in cells
    Fixed(u16),
    /// Percentage of parent
    Percent(f32),
    /// Flex grow factor
    Flex(u16),
    /// Auto size based on content
    #[default]
    Auto,
}

/// Layout properties for a node
#[derive(Debug, Clone, Default)]
pub struct Layout {
    /// Width
    pub width: Size,
    /// Height
    pub height: Size,
    /// Minimum width
    pub min_width: Option<u16>,
    /// Maximum width
    pub max_width: Option<u16>,
    /// Minimum height
    pub min_height: Option<u16>,
    /// Maximum height
    pub max_height: Option<u16>,
    /// Flex grow factor
    pub flex: Option<u16>,
    /// Padding (all sides)
    pub padding: u16,
    /// Padding horizontal
    pub padding_h: Option<u16>,
    /// Padding vertical
    pub padding_v: Option<u16>,
    /// Margin (all sides)
    pub margin: u16,
    /// Gap between children
    pub gap: u16,
    /// Content justification (main axis)
    pub justify: Justify,
    /// Content alignment (cross axis)
    pub align: Align,
    /// Border style
    pub border: Border,
}

impl Layout {
    /// Returns the total border size (both sides combined).
    /// Returns 2 for any border style except None, which returns 0.
    #[inline]
    pub fn border_size(&self) -> u16 {
        if matches!(self.border, Border::None) {
            0
        } else {
            2
        }
    }

    /// Returns the total horizontal padding (both sides combined).
    #[inline]
    pub fn padding_horizontal(&self) -> u16 {
        self.padding_h.unwrap_or(self.padding) * 2
    }

    /// Returns the total vertical padding (both sides combined).
    #[inline]
    pub fn padding_vertical(&self) -> u16 {
        self.padding_v.unwrap_or(self.padding) * 2
    }

    /// Returns the total chrome size (border + padding) for both axes.
    /// Returns (horizontal_chrome, vertical_chrome).
    #[inline]
    pub fn chrome_size(&self) -> (u16, u16) {
        let border = self.border_size();
        (
            self.padding_horizontal() + border,
            self.padding_vertical() + border,
        )
    }
}
