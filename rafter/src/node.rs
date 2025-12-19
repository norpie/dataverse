use crate::style::Style;

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

/// A node in the view tree
#[derive(Debug, Clone, Default)]
pub enum Node {
    /// Empty node (renders nothing)
    #[default]
    Empty,

    /// Text content
    Text { content: String, style: Style },

    /// Container with vertical layout
    Column {
        children: Vec<Node>,
        style: Style,
        layout: Layout,
    },

    /// Container with horizontal layout
    Row {
        children: Vec<Node>,
        style: Style,
        layout: Layout,
    },

    /// Stack (z-axis layering)
    Stack {
        children: Vec<Node>,
        style: Style,
        layout: Layout,
    },
}

impl Node {
    /// Create an empty node
    pub const fn empty() -> Self {
        Self::Empty
    }

    /// Create a text node
    pub fn text(content: impl Into<String>) -> Self {
        Self::Text {
            content: content.into(),
            style: Style::new(),
        }
    }

    /// Create a text node with style
    pub fn text_styled(content: impl Into<String>, style: Style) -> Self {
        Self::Text {
            content: content.into(),
            style,
        }
    }

    /// Create a column node
    pub fn column(children: Vec<Node>) -> Self {
        Self::Column {
            children,
            style: Style::new(),
            layout: Layout::default(),
        }
    }

    /// Create a column node with style and layout
    pub fn column_styled(children: Vec<Node>, style: Style, layout: Layout) -> Self {
        Self::Column {
            children,
            style,
            layout,
        }
    }

    /// Create a row node
    pub fn row(children: Vec<Node>) -> Self {
        Self::Row {
            children,
            style: Style::new(),
            layout: Layout::default(),
        }
    }

    /// Create a row node with style and layout
    pub fn row_styled(children: Vec<Node>, style: Style, layout: Layout) -> Self {
        Self::Row {
            children,
            style,
            layout,
        }
    }

    /// Check if node is empty
    pub fn is_empty(&self) -> bool {
        matches!(self, Self::Empty)
    }
}
