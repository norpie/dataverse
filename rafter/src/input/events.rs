/// Modifier keys state
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Modifiers {
    /// Control key held
    pub ctrl: bool,
    /// Shift key held
    pub shift: bool,
    /// Alt key held
    pub alt: bool,
}

impl Modifiers {
    /// No modifiers
    pub const NONE: Self = Self {
        ctrl: false,
        shift: false,
        alt: false,
    };

    /// Check if any modifier is active
    pub fn any(&self) -> bool {
        self.ctrl || self.shift || self.alt
    }
}

/// Click event kind
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClickKind {
    /// Primary action (Enter, left click)
    Primary,
    /// Secondary action (Shift+Enter, right click)
    Secondary,
}

/// Position in terminal cells
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Position {
    /// Column (0-indexed)
    pub x: u16,
    /// Row (0-indexed)
    pub y: u16,
}

impl Position {
    /// Create a new position
    pub const fn new(x: u16, y: u16) -> Self {
        Self { x, y }
    }
}

/// Click event from mouse or keyboard activation
#[derive(Debug, Clone)]
pub struct ClickEvent {
    /// Type of click
    pub kind: ClickKind,
    /// Position where click occurred (for mouse)
    pub position: Position,
    /// Modifier keys held during click
    pub modifiers: Modifiers,
}

impl ClickEvent {
    /// Create a primary click event
    pub fn primary(position: Position, modifiers: Modifiers) -> Self {
        Self {
            kind: ClickKind::Primary,
            position,
            modifiers,
        }
    }

    /// Create a secondary click event
    pub fn secondary(position: Position, modifiers: Modifiers) -> Self {
        Self {
            kind: ClickKind::Secondary,
            position,
            modifiers,
        }
    }
}

/// Text input change event
#[derive(Debug, Clone)]
pub struct InputEvent {
    /// Current input value
    pub value: String,
    /// Modifier keys held
    pub modifiers: Modifiers,
}

/// Text input submission event
#[derive(Debug, Clone)]
pub struct SubmitEvent {
    /// Submitted value
    pub value: String,
}

/// Scroll direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollDirection {
    Up,
    Down,
    Left,
    Right,
}

/// Scroll event from mouse wheel
#[derive(Debug, Clone)]
pub struct ScrollEvent {
    /// Scroll direction
    pub direction: ScrollDirection,
    /// Position where scroll occurred
    pub position: Position,
    /// Number of lines/columns to scroll
    pub amount: u16,
}
