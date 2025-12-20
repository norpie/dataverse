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

/// View scope for a keybind.
///
/// Keybinds can be scoped to specific views, meaning they are only active
/// when the app's `current_view()` returns a matching value.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub enum KeybindScope {
    /// Always active (no view restriction)
    #[default]
    Global,
    /// Only active when `current_view()` returns this view name
    View(String),
}

/// A single keybind entry (may be a sequence like "gg")
#[derive(Debug, Clone)]
pub struct Keybind {
    /// Key sequence to match (single key or multi-key sequence)
    pub keys: Vec<KeyCombo>,
    /// Handler to invoke
    pub handler: HandlerId,
    /// View scope for this keybind
    pub scope: KeybindScope,
}

impl Keybind {
    /// Create a single-key keybind (global scope)
    pub fn single(key: KeyCombo, handler: impl Into<HandlerId>) -> Self {
        Self {
            keys: vec![key],
            handler: handler.into(),
            scope: KeybindScope::Global,
        }
    }

    /// Create a multi-key sequence keybind (global scope)
    pub fn sequence(keys: Vec<KeyCombo>, handler: impl Into<HandlerId>) -> Self {
        Self {
            keys,
            handler: handler.into(),
            scope: KeybindScope::Global,
        }
    }

    /// Create a single-key keybind with view scope
    pub fn single_scoped(key: KeyCombo, handler: impl Into<HandlerId>, view: impl Into<String>) -> Self {
        Self {
            keys: vec![key],
            handler: handler.into(),
            scope: KeybindScope::View(view.into()),
        }
    }

    /// Create a multi-key sequence keybind with view scope
    pub fn sequence_scoped(keys: Vec<KeyCombo>, handler: impl Into<HandlerId>, view: impl Into<String>) -> Self {
        Self {
            keys,
            handler: handler.into(),
            scope: KeybindScope::View(view.into()),
        }
    }

    /// Set the view scope for this keybind
    pub fn with_scope(mut self, scope: KeybindScope) -> Self {
        self.scope = scope;
        self
    }

    /// Check if this keybind is active for the given view
    pub fn is_active_for(&self, current_view: Option<&str>) -> bool {
        match &self.scope {
            KeybindScope::Global => true,
            KeybindScope::View(view) => current_view == Some(view.as_str()),
        }
    }
}

/// Collection of keybinds
#[derive(Debug, Clone, Default)]
pub struct Keybinds {
    /// All registered keybinds
    binds: Vec<Keybind>,
}

impl Keybinds {
    /// Create empty keybinds
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a keybind
    pub fn add(&mut self, keybind: Keybind) {
        self.binds.push(keybind);
    }

    /// Add a simple key -> handler binding (global scope)
    pub fn bind(&mut self, key: KeyCombo, handler: impl Into<HandlerId>) {
        self.add(Keybind::single(key, handler));
    }

    /// Add a simple key -> handler binding with view scope
    pub fn bind_scoped(&mut self, key: KeyCombo, handler: impl Into<HandlerId>, view: impl Into<String>) {
        self.add(Keybind::single_scoped(key, handler, view));
    }

    /// Look up handler for a single key, respecting view scope
    pub fn get_single(&self, key: &KeyCombo, current_view: Option<&str>) -> Option<&HandlerId> {
        // First try view-scoped keybinds (higher priority)
        for bind in &self.binds {
            if bind.keys.len() == 1
                && bind.keys[0] == *key
                && let KeybindScope::View(view) = &bind.scope
                && current_view == Some(view.as_str())
            {
                return Some(&bind.handler);
            }
        }
        // Then try global keybinds
        for bind in &self.binds {
            if bind.keys.len() == 1
                && bind.keys[0] == *key
                && bind.scope == KeybindScope::Global
            {
                return Some(&bind.handler);
            }
        }
        None
    }

    /// Get all keybinds for sequence matching
    pub fn all(&self) -> &[Keybind] {
        &self.binds
    }

    /// Get keybinds that are active for the current view
    pub fn active_for(&self, current_view: Option<&str>) -> impl Iterator<Item = &Keybind> {
        self.binds.iter().filter(move |bind| bind.is_active_for(current_view))
    }

    /// Merge another keybinds collection into this one
    pub fn merge(&mut self, other: Keybinds) {
        for bind in other.binds {
            self.add(bind);
        }
    }

    /// Set the scope for all keybinds in this collection
    pub fn with_scope(mut self, scope: KeybindScope) -> Self {
        for bind in &mut self.binds {
            bind.scope = scope.clone();
        }
        self
    }
}
