/// High-level events with element targeting
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// Key press event, targeted at focused element
    Key {
        target: Option<String>,
        key: Key,
        modifiers: Modifiers,
    },
    /// Mouse click event
    Click {
        target: Option<String>,
        x: u16,
        y: u16,
        button: MouseButton,
    },
    /// Mouse scroll event
    Scroll {
        target: Option<String>,
        x: u16,
        y: u16,
        delta_x: i16,
        delta_y: i16,
    },
    /// Mouse move event (for hover tracking)
    MouseMove { x: u16, y: u16 },
    /// Mouse drag event (button held while moving)
    Drag {
        target: Option<String>,
        x: u16,
        y: u16,
        button: MouseButton,
    },
    /// Mouse button release event
    Release {
        target: Option<String>,
        x: u16,
        y: u16,
        button: MouseButton,
    },
    /// Element gained focus
    Focus { target: String },
    /// Element lost focus
    Blur { target: String },
    /// Terminal resized
    Resize { width: u16, height: u16 },
}

/// Simplified key representation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    Char(char),
    Enter,
    Backspace,
    Delete,
    Tab,
    BackTab,
    Escape,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    Insert,
    F(u8),
}

/// Key modifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

impl Modifiers {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn shift() -> Self {
        Self {
            shift: true,
            ..Default::default()
        }
    }

    pub fn ctrl() -> Self {
        Self {
            ctrl: true,
            ..Default::default()
        }
    }

    pub fn alt() -> Self {
        Self {
            alt: true,
            ..Default::default()
        }
    }

    pub fn none(&self) -> bool {
        !self.shift && !self.ctrl && !self.alt
    }
}

/// Mouse button
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

// Conversion from crossterm types
impl From<crossterm::event::KeyCode> for Key {
    fn from(code: crossterm::event::KeyCode) -> Self {
        use crossterm::event::KeyCode;
        match code {
            KeyCode::Char(c) => Key::Char(c),
            KeyCode::Enter => Key::Enter,
            KeyCode::Backspace => Key::Backspace,
            KeyCode::Delete => Key::Delete,
            KeyCode::Tab => Key::Tab,
            KeyCode::BackTab => Key::BackTab,
            KeyCode::Esc => Key::Escape,
            KeyCode::Up => Key::Up,
            KeyCode::Down => Key::Down,
            KeyCode::Left => Key::Left,
            KeyCode::Right => Key::Right,
            KeyCode::Home => Key::Home,
            KeyCode::End => Key::End,
            KeyCode::PageUp => Key::PageUp,
            KeyCode::PageDown => Key::PageDown,
            KeyCode::Insert => Key::Insert,
            KeyCode::F(n) => Key::F(n),
            _ => Key::Char('\0'), // Placeholder for unsupported keys
        }
    }
}

impl From<crossterm::event::KeyModifiers> for Modifiers {
    fn from(mods: crossterm::event::KeyModifiers) -> Self {
        use crossterm::event::KeyModifiers;
        Self {
            shift: mods.contains(KeyModifiers::SHIFT),
            ctrl: mods.contains(KeyModifiers::CONTROL),
            alt: mods.contains(KeyModifiers::ALT),
        }
    }
}

impl From<crossterm::event::MouseButton> for MouseButton {
    fn from(btn: crossterm::event::MouseButton) -> Self {
        use crossterm::event::MouseButton as CtBtn;
        match btn {
            CtBtn::Left => MouseButton::Left,
            CtBtn::Right => MouseButton::Right,
            CtBtn::Middle => MouseButton::Middle,
        }
    }
}
