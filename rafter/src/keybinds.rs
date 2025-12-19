use std::collections::HashMap;

use crate::events::Modifiers;

/// A key combination (key + modifiers)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeyCombo {
    /// The key code
    pub key: Key,
    /// Modifier keys
    pub modifiers: Modifiers,
}

impl KeyCombo {
    /// Create a new key combo
    pub const fn new(key: Key, modifiers: Modifiers) -> Self {
        Self { key, modifiers }
    }

    /// Create a key combo without modifiers
    pub const fn key(key: Key) -> Self {
        Self {
            key,
            modifiers: Modifiers::NONE,
        }
    }

    /// Add ctrl modifier
    pub const fn ctrl(mut self) -> Self {
        self.modifiers.ctrl = true;
        self
    }

    /// Add shift modifier
    pub const fn shift(mut self) -> Self {
        self.modifiers.shift = true;
        self
    }

    /// Add alt modifier
    pub const fn alt(mut self) -> Self {
        self.modifiers.alt = true;
        self
    }
}

/// Key codes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    /// Character key
    Char(char),
    /// Function keys F1-F12
    F(u8),
    /// Enter/Return
    Enter,
    /// Escape
    Escape,
    /// Backspace
    Backspace,
    /// Tab
    Tab,
    /// Space
    Space,
    /// Arrow up
    Up,
    /// Arrow down
    Down,
    /// Arrow left
    Left,
    /// Arrow right
    Right,
    /// Home
    Home,
    /// End
    End,
    /// Page up
    PageUp,
    /// Page down
    PageDown,
    /// Insert
    Insert,
    /// Delete
    Delete,
}

impl Key {
    /// Create a character key
    pub const fn char(c: char) -> Self {
        Self::Char(c)
    }
}

/// Handler identifier (used to reference handler methods)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HandlerId(pub String);

impl HandlerId {
    /// Create a new handler ID
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }
}

impl From<&str> for HandlerId {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

/// A single keybind entry (may be a sequence like "gg")
#[derive(Debug, Clone)]
pub struct Keybind {
    /// Key sequence to match (single key or multi-key sequence)
    pub keys: Vec<KeyCombo>,
    /// Handler to invoke
    pub handler: HandlerId,
}

impl Keybind {
    /// Create a single-key keybind
    pub fn single(key: KeyCombo, handler: impl Into<HandlerId>) -> Self {
        Self {
            keys: vec![key],
            handler: handler.into(),
        }
    }

    /// Create a multi-key sequence keybind
    pub fn sequence(keys: Vec<KeyCombo>, handler: impl Into<HandlerId>) -> Self {
        Self {
            keys,
            handler: handler.into(),
        }
    }
}

/// Collection of keybinds
#[derive(Debug, Clone, Default)]
pub struct Keybinds {
    /// All registered keybinds
    binds: Vec<Keybind>,
    /// Quick lookup for single-key binds
    single_key_map: HashMap<KeyCombo, HandlerId>,
}

impl Keybinds {
    /// Create empty keybinds
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a keybind
    pub fn add(&mut self, keybind: Keybind) {
        if keybind.keys.len() == 1 {
            self.single_key_map
                .insert(keybind.keys[0].clone(), keybind.handler.clone());
        }
        self.binds.push(keybind);
    }

    /// Add a simple key -> handler binding
    pub fn bind(&mut self, key: KeyCombo, handler: impl Into<HandlerId>) {
        self.add(Keybind::single(key, handler));
    }

    /// Look up handler for a single key
    pub fn get_single(&self, key: &KeyCombo) -> Option<&HandlerId> {
        self.single_key_map.get(key)
    }

    /// Get all keybinds for sequence matching
    pub fn all(&self) -> &[Keybind] {
        &self.binds
    }

    /// Merge another keybinds collection into this one
    pub fn merge(&mut self, other: Keybinds) {
        for bind in other.binds {
            self.add(bind);
        }
    }
}
