use crate::input::Input;
use crate::keybinds::HandlerId;
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

    /// Text input field
    Input {
        /// Current input value
        value: String,
        /// Placeholder text
        placeholder: String,
        /// Handler for value changes
        on_change: Option<HandlerId>,
        /// Handler for submit (Enter)
        on_submit: Option<HandlerId>,
        /// Element ID for focus (auto-generated if not specified)
        id: String,
        /// Style
        style: Style,
        /// Bound Input widget (if using bind: syntax)
        widget: Option<Input>,
    },

    /// Clickable button
    Button {
        /// Button label
        label: String,
        /// Click handler
        on_click: Option<HandlerId>,
        /// Element ID for focus (auto-generated if not specified)
        id: String,
        /// Style
        style: Style,
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

    /// Create a button node
    pub fn button(label: impl Into<String>) -> Self {
        Self::Button {
            label: label.into(),
            on_click: None,
            id: String::new(),
            style: Style::new(),
        }
    }

    /// Check if this node is focusable
    pub fn is_focusable(&self) -> bool {
        matches!(self, Self::Input { .. } | Self::Button { .. })
    }

    /// Get the element ID if any
    pub fn id(&self) -> Option<&str> {
        match self {
            Self::Input { id, .. } | Self::Button { id, .. } => {
                if id.is_empty() {
                    None
                } else {
                    Some(id.as_str())
                }
            }
            _ => None,
        }
    }

    /// Check if this node captures text input when focused
    pub fn captures_input(&self) -> bool {
        matches!(self, Self::Input { .. })
    }

    /// Collect all focusable element IDs from this node and its children (in tree order)
    pub fn collect_focusable_ids(&self, ids: &mut Vec<String>) {
        match self {
            Self::Input { id, .. } | Self::Button { id, .. } if !id.is_empty() => {
                ids.push(id.clone());
            }
            Self::Column { children, .. }
            | Self::Row { children, .. }
            | Self::Stack { children, .. } => {
                for child in children {
                    child.collect_focusable_ids(ids);
                }
            }
            _ => {}
        }
    }

    /// Get all focusable element IDs in tree order
    pub fn focusable_ids(&self) -> Vec<String> {
        let mut ids = Vec::new();
        self.collect_focusable_ids(&mut ids);
        ids
    }

    /// Check if an element with the given ID captures text input
    pub fn element_captures_input(&self, target_id: &str) -> bool {
        match self {
            Self::Input { id, .. } if id == target_id => true,
            Self::Column { children, .. }
            | Self::Row { children, .. }
            | Self::Stack { children, .. } => {
                children.iter().any(|c| c.element_captures_input(target_id))
            }
            _ => false,
        }
    }

    /// Get the current value of an input element by ID
    pub fn input_value(&self, target_id: &str) -> Option<String> {
        match self {
            Self::Input { id, value, .. } if id == target_id => Some(value.clone()),
            Self::Column { children, .. }
            | Self::Row { children, .. }
            | Self::Stack { children, .. } => {
                children.iter().find_map(|c| c.input_value(target_id))
            }
            _ => None,
        }
    }

    /// Get the handler for a focusable element (on_click for buttons, on_submit for inputs)
    pub fn get_submit_handler(&self, target_id: &str) -> Option<HandlerId> {
        match self {
            Self::Button { id, on_click, .. } if id == target_id => on_click.clone(),
            Self::Input { id, on_submit, .. } if id == target_id => on_submit.clone(),
            Self::Column { children, .. }
            | Self::Row { children, .. }
            | Self::Stack { children, .. } => children
                .iter()
                .find_map(|c| c.get_submit_handler(target_id)),
            _ => None,
        }
    }

    /// Get the on_change handler for an input element by ID
    pub fn get_change_handler(&self, target_id: &str) -> Option<HandlerId> {
        match self {
            Self::Input { id, on_change, .. } if id == target_id => on_change.clone(),
            Self::Column { children, .. }
            | Self::Row { children, .. }
            | Self::Stack { children, .. } => children
                .iter()
                .find_map(|c| c.get_change_handler(target_id)),
            _ => None,
        }
    }

    /// Get the Input widget for an input element by ID
    pub fn get_input_widget(&self, target_id: &str) -> Option<&Input> {
        match self {
            Self::Input { id, widget, .. } if id == target_id => widget.as_ref(),
            Self::Column { children, .. }
            | Self::Row { children, .. }
            | Self::Stack { children, .. } => children
                .iter()
                .find_map(|c| c.get_input_widget(target_id)),
            _ => None,
        }
    }

    /// Calculate intrinsic width of this node
    pub fn intrinsic_width(&self) -> u16 {
        match self {
            Self::Empty => 0,
            Self::Text { content, .. } => content.len() as u16,
            Self::Column {
                children, layout, ..
            } => {
                let border_size = if matches!(layout.border, Border::None) {
                    0
                } else {
                    2
                };
                let padding = layout.padding * 2;
                let max_child = children
                    .iter()
                    .map(|c| c.intrinsic_width())
                    .max()
                    .unwrap_or(0);
                max_child + padding + border_size
            }
            Self::Row {
                children, layout, ..
            } => {
                let border_size = if matches!(layout.border, Border::None) {
                    0
                } else {
                    2
                };
                let padding = layout.padding * 2;
                let child_sum: u16 = children.iter().map(|c| c.intrinsic_width()).sum();
                let gaps = if children.len() > 1 {
                    layout.gap * (children.len() as u16 - 1)
                } else {
                    0
                };
                child_sum + gaps + padding + border_size
            }
            Self::Stack {
                children, layout, ..
            } => {
                let border_size = if matches!(layout.border, Border::None) {
                    0
                } else {
                    2
                };
                let padding = layout.padding * 2;
                let max_child = children
                    .iter()
                    .map(|c| c.intrinsic_width())
                    .max()
                    .unwrap_or(0);
                max_child + padding + border_size
            }
            Self::Input {
                value, placeholder, ..
            } => {
                let content_len = if value.is_empty() {
                    placeholder.len()
                } else {
                    value.len()
                };
                (content_len + 5).max(15) as u16
            }
            Self::Button { label, .. } => (label.len() + 4) as u16,
        }
    }

    /// Calculate intrinsic height of this node
    pub fn intrinsic_height(&self) -> u16 {
        match self {
            Self::Empty => 0,
            Self::Text { content, .. } => content.lines().count().max(1) as u16,
            Self::Column {
                children, layout, ..
            } => {
                let border_size = if matches!(layout.border, Border::None) {
                    0
                } else {
                    2
                };
                let padding = layout.padding * 2;
                let child_sum: u16 = children.iter().map(|c| c.intrinsic_height()).sum();
                let gaps = if children.len() > 1 {
                    layout.gap * (children.len() as u16 - 1)
                } else {
                    0
                };
                child_sum + gaps + padding + border_size
            }
            Self::Row {
                children, layout, ..
            } => {
                let border_size = if matches!(layout.border, Border::None) {
                    0
                } else {
                    2
                };
                let padding = layout.padding * 2;
                let max_child = children
                    .iter()
                    .map(|c| c.intrinsic_height())
                    .max()
                    .unwrap_or(0);
                max_child + padding + border_size
            }
            Self::Stack {
                children, layout, ..
            } => {
                let border_size = if matches!(layout.border, Border::None) {
                    0
                } else {
                    2
                };
                let padding = layout.padding * 2;
                let max_child = children
                    .iter()
                    .map(|c| c.intrinsic_height())
                    .max()
                    .unwrap_or(0);
                max_child + padding + border_size
            }
            Self::Input { .. } | Self::Button { .. } => 1,
        }
    }
}
