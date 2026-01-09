use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use super::{Content, CustomContent};
use crate::transitions::Transitions;
use crate::types::{
    Align, Backdrop, Direction, Edges, Justify, Overflow, Position, Size, Style, TextAlign,
    TextWrap, Wrap,
};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

fn generate_id(prefix: &str) -> String {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    format!("{prefix}-{id}")
}

#[derive(Debug, Clone)]
pub struct Element {
    // Identity
    pub id: String,

    // Content
    pub content: Content,

    // Layout (box model)
    pub width: Size,
    pub height: Size,
    pub min_width: Option<u16>,
    pub max_width: Option<u16>,
    pub min_height: Option<u16>,
    pub max_height: Option<u16>,
    pub padding: Edges,
    pub margin: Edges,

    // Positioning
    pub position: Position,
    pub top: Option<i16>,
    pub left: Option<i16>,
    pub right: Option<i16>,
    pub bottom: Option<i16>,
    pub z_index: i16,

    // Flex container
    pub direction: Direction,
    pub gap: u16,
    pub justify: Justify,
    pub align: Align,
    pub wrap: Wrap,

    // Flex item
    pub flex_grow: u16,
    pub flex_shrink: u16,
    pub align_self: Option<Align>,

    // Overflow
    pub overflow: Overflow,
    pub scroll_offset: (u16, u16),

    // Visual
    pub style: Style,
    pub transitions: Transitions,
    pub backdrop: Backdrop,

    // Text-specific
    pub text_wrap: TextWrap,
    pub text_align: TextAlign,

    // Interaction
    pub focusable: bool,
    pub clickable: bool,
    pub draggable: bool,
    /// When true, this element captures keyboard input (for text fields).
    /// Arrow keys will move cursor instead of focus, etc.
    pub captures_input: bool,

    // State (focused is set by runtime enrichment, disabled is set by user/widgets)
    /// Whether this element is currently focused. Set by runtime enrichment, not by user.
    pub focused: bool,
    /// Whether this element is disabled. Disabled elements don't receive input.
    pub disabled: bool,

    // State-dependent styles (set by user/widgets, applied by runtime enrichment)
    pub style_focused: Option<Style>,
    pub style_disabled: Option<Style>,

    // Custom data storage (for handler IDs, etc.)
    pub data: HashMap<String, String>,
}

impl Default for Element {
    fn default() -> Self {
        Self {
            id: generate_id("el"),
            content: Content::None,
            width: Size::Auto,
            height: Size::Auto,
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            padding: Edges::default(),
            margin: Edges::default(),
            position: Position::Static,
            top: None,
            left: None,
            right: None,
            bottom: None,
            z_index: 0,
            direction: Direction::Column,
            gap: 0,
            justify: Justify::Start,
            align: Align::Start,
            wrap: Wrap::NoWrap,
            flex_grow: 0,
            flex_shrink: 1,
            align_self: None,
            overflow: Overflow::Visible,
            scroll_offset: (0, 0),
            style: Style::default(),
            transitions: Transitions::default(),
            backdrop: Backdrop::None,
            text_wrap: TextWrap::NoWrap,
            text_align: TextAlign::Left,
            focusable: false,
            clickable: false,
            draggable: false,
            captures_input: false,
            focused: false,
            disabled: false,
            style_focused: None,
            style_disabled: None,
            data: HashMap::new(),
        }
    }
}

impl Element {
    pub fn box_() -> Self {
        Self {
            id: generate_id("box"),
            ..Default::default()
        }
    }

    pub fn text(content: impl Into<String>) -> Self {
        Self {
            id: generate_id("text"),
            content: Content::Text(content.into()),
            ..Default::default()
        }
    }

    pub fn col() -> Self {
        Self {
            id: generate_id("col"),
            direction: Direction::Column,
            ..Default::default()
        }
    }

    pub fn row() -> Self {
        Self {
            id: generate_id("row"),
            direction: Direction::Row,
            ..Default::default()
        }
    }

    pub fn custom(content: impl CustomContent + 'static) -> Self {
        Self {
            id: generate_id("custom"),
            content: Content::Custom(Box::new(content)),
            ..Default::default()
        }
    }

    /// Create a text input element.
    pub fn text_input(value: impl Into<String>) -> Self {
        Self {
            id: generate_id("input"),
            content: Content::TextInput {
                value: value.into(),
                cursor: 0,
                selection: None,
                placeholder: None,
                focused: false,
                mask: None,
            },
            focusable: true,
            captures_input: true,
            ..Default::default()
        }
    }

    /// Create an element that cycles through child frames at the given interval.
    /// Only the current frame is laid out and rendered at any time.
    pub fn frames(children: Vec<Element>, interval: Duration) -> Self {
        Self {
            id: generate_id("frames"),
            content: Content::Frames { children, interval },
            ..Default::default()
        }
    }

    // Identity
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = id.into();
        self
    }

    // Layout
    pub fn width(mut self, width: Size) -> Self {
        self.width = width;
        self
    }

    pub fn height(mut self, height: Size) -> Self {
        self.height = height;
        self
    }

    pub fn min_width(mut self, min_width: u16) -> Self {
        self.min_width = Some(min_width);
        self
    }

    pub fn max_width(mut self, max_width: u16) -> Self {
        self.max_width = Some(max_width);
        self
    }

    pub fn min_height(mut self, min_height: u16) -> Self {
        self.min_height = Some(min_height);
        self
    }

    pub fn max_height(mut self, max_height: u16) -> Self {
        self.max_height = Some(max_height);
        self
    }

    pub fn padding(mut self, padding: Edges) -> Self {
        self.padding = padding;
        self
    }

    pub fn margin(mut self, margin: Edges) -> Self {
        self.margin = margin;
        self
    }

    // Positioning
    pub fn position(mut self, position: Position) -> Self {
        self.position = position;
        self
    }

    pub fn top(mut self, top: i16) -> Self {
        self.top = Some(top);
        self
    }

    pub fn left(mut self, left: i16) -> Self {
        self.left = Some(left);
        self
    }

    pub fn right(mut self, right: i16) -> Self {
        self.right = Some(right);
        self
    }

    pub fn bottom(mut self, bottom: i16) -> Self {
        self.bottom = Some(bottom);
        self
    }

    pub fn z_index(mut self, z_index: i16) -> Self {
        self.z_index = z_index;
        self
    }

    // Flex container
    pub fn direction(mut self, direction: Direction) -> Self {
        self.direction = direction;
        self
    }

    pub fn gap(mut self, gap: u16) -> Self {
        self.gap = gap;
        self
    }

    pub fn justify(mut self, justify: Justify) -> Self {
        self.justify = justify;
        self
    }

    pub fn align(mut self, align: Align) -> Self {
        self.align = align;
        self
    }

    pub fn wrap(mut self, wrap: Wrap) -> Self {
        self.wrap = wrap;
        self
    }

    // Flex item
    pub fn flex_grow(mut self, flex_grow: u16) -> Self {
        self.flex_grow = flex_grow;
        self
    }

    pub fn flex_shrink(mut self, flex_shrink: u16) -> Self {
        self.flex_shrink = flex_shrink;
        self
    }

    pub fn align_self(mut self, align_self: Align) -> Self {
        self.align_self = Some(align_self);
        self
    }

    // Overflow
    pub fn overflow(mut self, overflow: Overflow) -> Self {
        self.overflow = overflow;
        self
    }

    pub fn scroll_offset(mut self, x: u16, y: u16) -> Self {
        self.scroll_offset = (x, y);
        self
    }

    // Visual
    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    pub fn transitions(mut self, transitions: Transitions) -> Self {
        self.transitions = transitions;
        self
    }

    pub fn backdrop(mut self, backdrop: Backdrop) -> Self {
        self.backdrop = backdrop;
        self
    }

    // Text
    pub fn text_wrap(mut self, text_wrap: TextWrap) -> Self {
        self.text_wrap = text_wrap;
        self
    }

    pub fn text_align(mut self, text_align: TextAlign) -> Self {
        self.text_align = text_align;
        self
    }

    // Interaction
    pub fn focusable(mut self, focusable: bool) -> Self {
        self.focusable = focusable;
        self
    }

    pub fn clickable(mut self, clickable: bool) -> Self {
        self.clickable = clickable;
        self
    }

    pub fn draggable(mut self, draggable: bool) -> Self {
        self.draggable = draggable;
        self
    }

    pub fn captures_input(mut self, captures: bool) -> Self {
        self.captures_input = captures;
        self
    }

    // State
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn style_focused(mut self, style: Style) -> Self {
        self.style_focused = Some(style);
        self
    }

    pub fn style_disabled(mut self, style: Style) -> Self {
        self.style_disabled = Some(style);
        self
    }

    // Text input methods

    /// Set the cursor position for a text input.
    pub fn cursor(mut self, position: usize) -> Self {
        if let Content::TextInput { cursor, .. } = &mut self.content {
            *cursor = position;
        }
        self
    }

    /// Set the selection range for a text input.
    pub fn selection(mut self, range: Option<(usize, usize)>) -> Self {
        if let Content::TextInput { selection, .. } = &mut self.content {
            *selection = range;
        }
        self
    }

    /// Set the placeholder text for a text input.
    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        if let Content::TextInput { placeholder, .. } = &mut self.content {
            *placeholder = Some(text.into());
        }
        self
    }

    /// Set whether the text input is focused (shows cursor).
    pub fn input_focused(mut self, is_focused: bool) -> Self {
        if let Content::TextInput { focused, .. } = &mut self.content {
            *focused = is_focused;
        }
        self
    }

    /// Set all text input state from TextInputData.
    pub fn input_state(mut self, data: &crate::text_input::TextInputData, is_focused: bool) -> Self {
        if let Content::TextInput {
            value,
            cursor,
            selection,
            focused,
            ..
        } = &mut self.content
        {
            *value = data.text.clone();
            *cursor = data.cursor;
            *selection = data.selection();
            *focused = is_focused;
        }
        self
    }

    /// Set the text input to password mode (displays • for each character).
    pub fn password(mut self) -> Self {
        if let Content::TextInput { mask, .. } = &mut self.content {
            *mask = Some('•');
        }
        self
    }

    /// Set a custom mask character for the text input.
    pub fn masked(mut self, mask_char: char) -> Self {
        if let Content::TextInput { mask, .. } = &mut self.content {
            *mask = Some(mask_char);
        }
        self
    }

    // Custom data
    pub fn data(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.data.insert(key.into(), value.into());
        self
    }

    pub fn get_data(&self, key: &str) -> Option<&String> {
        self.data.get(key)
    }

    // Children
    pub fn child(mut self, child: Element) -> Self {
        match &mut self.content {
            Content::Children(children) => children.push(child),
            Content::None => self.content = Content::Children(vec![child]),
            _ => {
                // Replace content with children
                self.content = Content::Children(vec![child]);
            }
        }
        self
    }

    pub fn children(mut self, new_children: impl IntoIterator<Item = Element>) -> Self {
        match &mut self.content {
            Content::Children(children) => children.extend(new_children),
            Content::None => self.content = Content::Children(new_children.into_iter().collect()),
            _ => {
                self.content = Content::Children(new_children.into_iter().collect());
            }
        }
        self
    }
}
