use crate::events::Modifiers;

/// Error when parsing a key string
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseKeyError {
    pub message: String,
}

impl std::fmt::Display for ParseKeyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ParseKeyError {}

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

    /// Parse a key from a string like "enter", "escape", "a", "f1"
    pub fn parse(s: &str) -> Result<Self, ParseKeyError> {
        match s.to_lowercase().as_str() {
            "enter" | "return" => Ok(Key::Enter),
            "escape" | "esc" => Ok(Key::Escape),
            "backspace" => Ok(Key::Backspace),
            "tab" => Ok(Key::Tab),
            "space" => Ok(Key::Space),
            "up" => Ok(Key::Up),
            "down" => Ok(Key::Down),
            "left" => Ok(Key::Left),
            "right" => Ok(Key::Right),
            "home" => Ok(Key::Home),
            "end" => Ok(Key::End),
            "pageup" | "pgup" => Ok(Key::PageUp),
            "pagedown" | "pgdn" => Ok(Key::PageDown),
            "insert" | "ins" => Ok(Key::Insert),
            "delete" | "del" => Ok(Key::Delete),
            "f1" => Ok(Key::F(1)),
            "f2" => Ok(Key::F(2)),
            "f3" => Ok(Key::F(3)),
            "f4" => Ok(Key::F(4)),
            "f5" => Ok(Key::F(5)),
            "f6" => Ok(Key::F(6)),
            "f7" => Ok(Key::F(7)),
            "f8" => Ok(Key::F(8)),
            "f9" => Ok(Key::F(9)),
            "f10" => Ok(Key::F(10)),
            "f11" => Ok(Key::F(11)),
            "f12" => Ok(Key::F(12)),
            _ => {
                // Single character key
                let chars: Vec<char> = s.chars().collect();
                if chars.len() == 1 {
                    Ok(Key::Char(chars[0]))
                } else {
                    Err(ParseKeyError {
                        message: format!("Unknown key: {}", s),
                    })
                }
            }
        }
    }
}

/// Check if a string is a special key name
fn is_special_key(s: &str) -> bool {
    matches!(
        s.to_lowercase().as_str(),
        "enter"
            | "return"
            | "escape"
            | "esc"
            | "backspace"
            | "tab"
            | "space"
            | "up"
            | "down"
            | "left"
            | "right"
            | "home"
            | "end"
            | "pageup"
            | "pgup"
            | "pagedown"
            | "pgdn"
            | "insert"
            | "ins"
            | "delete"
            | "del"
            | "f1"
            | "f2"
            | "f3"
            | "f4"
            | "f5"
            | "f6"
            | "f7"
            | "f8"
            | "f9"
            | "f10"
            | "f11"
            | "f12"
    )
}

/// Parse a key string like "ctrl+shift+a" or "gg" into KeyCombo(s)
/// 
/// Supports:
/// - Single keys: "a", "enter", "f1"
/// - Modifiers: "ctrl+a", "ctrl+shift+a", "alt+f4"
/// - Sequences: "gg", "gc" (vim-style multi-key)
/// 
/// # Examples
/// ```ignore
/// let keys = parse_key_string("ctrl+s")?;  // [KeyCombo { key: Char('s'), modifiers: ctrl }]
/// let keys = parse_key_string("gg")?;      // [KeyCombo { key: Char('g') }, KeyCombo { key: Char('g') }]
/// ```
pub fn parse_key_string(s: &str) -> Result<Vec<KeyCombo>, ParseKeyError> {
    // Check for modifier prefixes
    let mut ctrl = false;
    let mut shift = false;
    let mut alt = false;

    let parts: Vec<&str> = s.split('+').collect();
    let key_part = if parts.len() > 1 {
        // Has modifiers
        for part in &parts[..parts.len() - 1] {
            match part.to_lowercase().as_str() {
                "ctrl" | "control" => ctrl = true,
                "shift" => shift = true,
                "alt" => alt = true,
                other => {
                    return Err(ParseKeyError {
                        message: format!("Unknown modifier: {}", other),
                    });
                }
            }
        }
        parts[parts.len() - 1]
    } else {
        parts[0]
    };

    // Check if it's a sequence (multiple chars without modifiers, like "gg")
    // but NOT a special key name
    let is_sequence = !ctrl
        && !shift
        && !alt
        && key_part.len() > 1
        && key_part.chars().all(|c| c.is_alphanumeric())
        && !is_special_key(key_part);

    if is_sequence {
        // Generate a sequence of KeyCombos
        let combos: Vec<KeyCombo> = key_part
            .chars()
            .map(|c| KeyCombo::new(Key::Char(c), Modifiers::NONE))
            .collect();
        Ok(combos)
    } else {
        // Single key with optional modifiers
        let key = Key::parse(key_part)?;
        let modifiers = Modifiers { ctrl, shift, alt };
        Ok(vec![KeyCombo::new(key, modifiers)])
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
    /// Unique identifier for configuration (e.g., "explorer_app.record_view.delete")
    /// Set by the macro based on app name, view scope, and handler name.
    pub id: String,
    /// The original key string from the macro (e.g., "ctrl+d", "gg")
    /// Used for displaying defaults and resetting overrides.
    pub default_keys: String,
    /// Current key sequence to match (may differ from default if overridden)
    /// None means the keybind is disabled.
    pub keys: Option<Vec<KeyCombo>>,
    /// Handler to invoke
    pub handler: HandlerId,
    /// View scope for this keybind
    pub scope: KeybindScope,
}

impl Keybind {
    /// Create a new keybind with all fields specified.
    /// This is the primary constructor used by the macro.
    pub fn new(
        id: impl Into<String>,
        default_keys: impl Into<String>,
        keys: Vec<KeyCombo>,
        handler: impl Into<HandlerId>,
    ) -> Self {
        Self {
            id: id.into(),
            default_keys: default_keys.into(),
            keys: Some(keys),
            handler: handler.into(),
            scope: KeybindScope::Global,
        }
    }

    /// Set the view scope for this keybind
    pub fn with_scope(mut self, scope: KeybindScope) -> Self {
        self.scope = scope;
        self
    }

    /// Set the ID prefix (prepends to existing ID)
    pub fn with_id_prefix(mut self, prefix: &str) -> Self {
        self.id = format!("{}.{}", prefix, self.id);
        self
    }

    /// Check if this keybind is enabled
    pub fn is_enabled(&self) -> bool {
        self.keys.is_some()
    }

    /// Check if this keybind is active for the given view
    pub fn is_active_for(&self, current_view: Option<&str>) -> bool {
        if !self.is_enabled() {
            return false;
        }
        match &self.scope {
            KeybindScope::Global => true,
            KeybindScope::View(view) => current_view == Some(view.as_str()),
        }
    }

    /// Get the current keys (if enabled)
    pub fn current_keys(&self) -> Option<&[KeyCombo]> {
        self.keys.as_deref()
    }

    /// Disable this keybind
    pub fn disable(&mut self) {
        self.keys = None;
    }

    /// Reset to default keys (requires parsing, done externally)
    pub fn set_keys(&mut self, keys: Vec<KeyCombo>) {
        self.keys = Some(keys);
    }

    /// Convert to a display-friendly info struct
    pub fn to_info(&self) -> KeybindInfo {
        KeybindInfo {
            id: self.id.clone(),
            default_keys: self.default_keys.clone(),
            current_keys: self.keys.as_ref().map(|_| self.default_keys.clone()), // TODO: serialize current keys
            handler: self.handler.0.clone(),
            scope: match &self.scope {
                KeybindScope::Global => None,
                KeybindScope::View(v) => Some(v.clone()),
            },
            enabled: self.is_enabled(),
        }
    }
}

/// Display-friendly information about a keybind.
/// 
/// Used for settings UI, help menus, etc.
#[derive(Debug, Clone)]
pub struct KeybindInfo {
    /// Unique identifier (e.g., "explorer_app.record_view.delete")
    pub id: String,
    /// Default key string from macro (e.g., "ctrl+d")
    pub default_keys: String,
    /// Current key string (None if disabled)
    pub current_keys: Option<String>,
    /// Handler name (e.g., "delete")
    pub handler: String,
    /// View scope name (None if global)
    pub scope: Option<String>,
    /// Whether the keybind is enabled
    pub enabled: bool,
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
    /// Note: This generates an ID from the handler name. For full control, use `add()` directly.
    pub fn bind(&mut self, key: KeyCombo, handler: impl Into<HandlerId>) {
        let handler = handler.into();
        let key_str = format!("{:?}", key); // Simple debug representation
        self.add(Keybind::new(
            handler.0.clone(),
            key_str,
            vec![key],
            handler,
        ));
    }

    /// Add a simple key -> handler binding with view scope
    /// Note: This generates an ID from the handler name. For full control, use `add()` directly.
    pub fn bind_scoped(&mut self, key: KeyCombo, handler: impl Into<HandlerId>, view: impl Into<String>) {
        let handler = handler.into();
        let view = view.into();
        let key_str = format!("{:?}", key); // Simple debug representation
        self.add(Keybind::new(
            handler.0.clone(),
            key_str,
            vec![key],
            handler,
        ).with_scope(KeybindScope::View(view)));
    }

    /// Look up handler for a single key, respecting view scope
    pub fn get_single(&self, key: &KeyCombo, current_view: Option<&str>) -> Option<&HandlerId> {
        // First try view-scoped keybinds (higher priority)
        for bind in &self.binds {
            if let Some(keys) = &bind.keys
                && keys.len() == 1
                && keys[0] == *key
                && let KeybindScope::View(view) = &bind.scope
                && current_view == Some(view.as_str())
            {
                return Some(&bind.handler);
            }
        }
        // Then try global keybinds
        for bind in &self.binds {
            if let Some(keys) = &bind.keys
                && keys.len() == 1
                && keys[0] == *key
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

    /// Set the ID prefix for all keybinds in this collection
    pub fn with_id_prefix(mut self, prefix: &str) -> Self {
        for bind in &mut self.binds {
            bind.id = format!("{}.{}", prefix, bind.id);
        }
        self
    }

    /// Find a keybind by ID
    pub fn get_by_id(&self, id: &str) -> Option<&Keybind> {
        self.binds.iter().find(|b| b.id == id)
    }

    /// Find a keybind by ID (mutable)
    pub fn get_by_id_mut(&mut self, id: &str) -> Option<&mut Keybind> {
        self.binds.iter_mut().find(|b| b.id == id)
    }

    /// Get all keybind IDs
    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.binds.iter().map(|b| b.id.as_str())
    }

    /// Get display info for all keybinds
    pub fn infos(&self) -> Vec<KeybindInfo> {
        self.binds.iter().map(|b| b.to_info()).collect()
    }

    /// Get display info for active keybinds only
    pub fn active_infos(&self, current_view: Option<&str>) -> Vec<KeybindInfo> {
        self.binds
            .iter()
            .filter(|b| b.is_active_for(current_view))
            .map(|b| b.to_info())
            .collect()
    }

    /// Override a keybind's keys by ID
    /// Returns an error if the ID is not found or the key string is invalid.
    pub fn override_keybind(&mut self, id: &str, key_string: &str) -> Result<(), KeybindError> {
        let bind = self
            .get_by_id_mut(id)
            .ok_or_else(|| KeybindError::NotFound(id.to_string()))?;

        let keys = parse_key_string(key_string).map_err(|e| KeybindError::ParseError(e.message))?;
        bind.set_keys(keys);
        Ok(())
    }

    /// Disable a keybind by ID
    pub fn disable_keybind(&mut self, id: &str) -> Result<(), KeybindError> {
        let bind = self
            .get_by_id_mut(id)
            .ok_or_else(|| KeybindError::NotFound(id.to_string()))?;

        bind.disable();
        Ok(())
    }

    /// Reset a keybind to its default keys
    pub fn reset_keybind(&mut self, id: &str) -> Result<(), KeybindError> {
        let bind = self
            .get_by_id_mut(id)
            .ok_or_else(|| KeybindError::NotFound(id.to_string()))?;

        let keys =
            parse_key_string(&bind.default_keys).map_err(|e| KeybindError::ParseError(e.message))?;
        bind.set_keys(keys);
        Ok(())
    }

    /// Reset all keybinds to their defaults
    pub fn reset_all(&mut self) {
        for bind in &mut self.binds {
            if let Ok(keys) = parse_key_string(&bind.default_keys) {
                bind.set_keys(keys);
            }
        }
    }
}

/// Errors that can occur when manipulating keybinds
#[derive(Debug, Clone)]
pub enum KeybindError {
    /// Keybind ID not found
    NotFound(String),
    /// Failed to parse key string
    ParseError(String),
}

impl std::fmt::Display for KeybindError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KeybindError::NotFound(id) => write!(f, "Keybind not found: {}", id),
            KeybindError::ParseError(msg) => write!(f, "Failed to parse key string: {}", msg),
        }
    }
}

impl std::error::Error for KeybindError {}
