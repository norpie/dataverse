//! Keybind system for mapping key combinations to handlers.

use tuidom::{Key, Modifiers};

/// Error when parsing a key string.
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

/// A key combination (key + modifiers).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeyCombo {
    pub key: Key,
    pub modifiers: Modifiers,
}

impl KeyCombo {
    pub const fn new(key: Key, modifiers: Modifiers) -> Self {
        Self { key, modifiers }
    }

    pub const fn key(key: Key) -> Self {
        Self {
            key,
            modifiers: Modifiers {
                ctrl: false,
                shift: false,
                alt: false,
            },
        }
    }

    pub const fn ctrl(mut self) -> Self {
        self.modifiers.ctrl = true;
        self
    }

    pub const fn shift(mut self) -> Self {
        self.modifiers.shift = true;
        self
    }

    pub const fn alt(mut self) -> Self {
        self.modifiers.alt = true;
        self
    }
}

/// Handler identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HandlerId(pub String);

impl HandlerId {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }
}

impl From<&str> for HandlerId {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

/// Page scope for a keybind.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub enum KeybindScope {
    /// Always active.
    #[default]
    Global,
    /// Only active when on this page.
    Page(String),
}

/// A single keybind entry.
#[derive(Debug, Clone)]
pub struct Keybind {
    /// Unique identifier for configuration (e.g., "my_app.record_view.delete").
    pub id: String,
    /// Original key string from macro (e.g., "ctrl+d", "gg").
    pub default_keys: String,
    /// Current key sequence (None = disabled).
    pub keys: Option<Vec<KeyCombo>>,
    /// Handler to invoke.
    pub handler: HandlerId,
    /// Page scope.
    pub scope: KeybindScope,
}

impl Keybind {
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

    pub fn with_scope(mut self, scope: KeybindScope) -> Self {
        self.scope = scope;
        self
    }

    pub fn with_id_prefix(mut self, prefix: &str) -> Self {
        self.id = format!("{}.{}", prefix, self.id);
        self
    }

    pub fn is_enabled(&self) -> bool {
        self.keys.is_some()
    }

    pub fn is_active_for(&self, current_page: Option<&str>) -> bool {
        if !self.is_enabled() {
            return false;
        }
        match &self.scope {
            KeybindScope::Global => true,
            KeybindScope::Page(page) => current_page == Some(page.as_str()),
        }
    }

    pub fn current_keys(&self) -> Option<&[KeyCombo]> {
        self.keys.as_deref()
    }

    pub fn disable(&mut self) {
        self.keys = None;
    }

    pub fn set_keys(&mut self, keys: Vec<KeyCombo>) {
        self.keys = Some(keys);
    }
}

/// Collection of keybinds.
#[derive(Debug, Clone, Default)]
pub struct Keybinds {
    binds: Vec<Keybind>,
}

impl Keybinds {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, keybind: Keybind) {
        self.binds.push(keybind);
    }

    /// Add a keybind from a key string and handler name.
    /// Returns false and logs if parsing fails.
    pub fn add_str(&mut self, key_string: &str, handler: &str) -> bool {
        match parse_key_string(key_string) {
            Ok(keys) => {
                self.binds.push(Keybind::new(handler, key_string, keys, handler));
                true
            }
            Err(e) => {
                log::error!(
                    "Failed to parse keybind '{}' for handler '{}': {}",
                    key_string,
                    handler,
                    e
                );
                false
            }
        }
    }

    /// Look up handler for a single key, respecting page scope.
    pub fn get_single(&self, key: &KeyCombo, current_page: Option<&str>) -> Option<&HandlerId> {
        // Page-scoped keybinds take priority
        for bind in &self.binds {
            if let Some(keys) = &bind.keys
                && keys.len() == 1
                && keys[0] == *key
                && let KeybindScope::Page(page) = &bind.scope
                && current_page == Some(page.as_str())
            {
                return Some(&bind.handler);
            }
        }
        // Then global keybinds
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

    pub fn all(&self) -> &[Keybind] {
        &self.binds
    }

    pub fn active_for(&self, current_page: Option<&str>) -> impl Iterator<Item = &Keybind> {
        self.binds
            .iter()
            .filter(move |bind| bind.is_active_for(current_page))
    }

    pub fn merge(&mut self, other: Keybinds) {
        for bind in other.binds {
            self.add(bind);
        }
    }

    pub fn with_scope(mut self, scope: KeybindScope) -> Self {
        for bind in &mut self.binds {
            bind.scope = scope.clone();
        }
        self
    }

    pub fn with_id_prefix(mut self, prefix: &str) -> Self {
        for bind in &mut self.binds {
            bind.id = format!("{}.{}", prefix, bind.id);
        }
        self
    }

    pub fn get_by_id(&self, id: &str) -> Option<&Keybind> {
        self.binds.iter().find(|b| b.id == id)
    }

    pub fn get_by_id_mut(&mut self, id: &str) -> Option<&mut Keybind> {
        self.binds.iter_mut().find(|b| b.id == id)
    }

    /// Override a keybind's keys by ID.
    pub fn override_keybind(&mut self, id: &str, key_string: &str) -> Result<(), KeybindError> {
        let bind = self
            .get_by_id_mut(id)
            .ok_or_else(|| KeybindError::NotFound(id.to_string()))?;

        let keys = parse_key_string(key_string).map_err(|e| KeybindError::ParseError(e.message))?;
        bind.set_keys(keys);
        Ok(())
    }

    /// Disable a keybind by ID.
    pub fn disable_keybind(&mut self, id: &str) -> Result<(), KeybindError> {
        let bind = self
            .get_by_id_mut(id)
            .ok_or_else(|| KeybindError::NotFound(id.to_string()))?;

        bind.disable();
        Ok(())
    }

    /// Reset a keybind to its default keys.
    pub fn reset_keybind(&mut self, id: &str) -> Result<(), KeybindError> {
        let bind = self
            .get_by_id_mut(id)
            .ok_or_else(|| KeybindError::NotFound(id.to_string()))?;

        let keys = parse_key_string(&bind.default_keys)
            .map_err(|e| KeybindError::ParseError(e.message))?;
        bind.set_keys(keys);
        Ok(())
    }

    /// Reset all keybinds to their defaults.
    pub fn reset_all(&mut self) {
        for bind in &mut self.binds {
            if let Ok(keys) = parse_key_string(&bind.default_keys) {
                bind.set_keys(keys);
            }
        }
    }

    /// Get display info for all keybinds.
    pub fn infos(&self) -> Vec<KeybindInfo> {
        self.binds
            .iter()
            .map(|bind| KeybindInfo {
                id: bind.id.clone(),
                keys: if bind.is_enabled() {
                    Some(bind.default_keys.clone())
                } else {
                    None
                },
                handler: bind.handler.0.clone(),
                enabled: bind.is_enabled(),
            })
            .collect()
    }
}

/// Errors when manipulating keybinds.
#[derive(Debug, Clone)]
pub enum KeybindError {
    NotFound(String),
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

/// Display info for a keybind (for UI display).
#[derive(Debug, Clone)]
pub struct KeybindInfo {
    /// Unique identifier.
    pub id: String,
    /// Current key string (None if disabled).
    pub keys: Option<String>,
    /// Handler name.
    pub handler: String,
    /// Whether the keybind is enabled.
    pub enabled: bool,
}

/// Parse a key string like "ctrl+shift+a" or "gg" into KeyCombo(s).
pub fn parse_key_string(s: &str) -> Result<Vec<KeyCombo>, ParseKeyError> {
    let mut ctrl = false;
    let mut shift = false;
    let mut alt = false;

    let parts: Vec<&str> = s.split('+').collect();
    let key_part = if parts.len() > 1 {
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

    // Check if it's a sequence (multiple chars without modifiers)
    let is_sequence = !ctrl
        && !shift
        && !alt
        && key_part.len() > 1
        && key_part.chars().all(|c| c.is_alphanumeric())
        && !is_special_key(key_part);

    if is_sequence {
        let combos: Vec<KeyCombo> = key_part
            .chars()
            .map(|c| KeyCombo::new(Key::Char(c), Modifiers::default()))
            .collect();
        Ok(combos)
    } else {
        let key = parse_key(key_part)?;
        let modifiers = Modifiers { ctrl, shift, alt };
        Ok(vec![KeyCombo::new(key, modifiers)])
    }
}

fn parse_key(s: &str) -> Result<Key, ParseKeyError> {
    match s.to_lowercase().as_str() {
        "enter" | "return" => Ok(Key::Enter),
        "escape" | "esc" => Ok(Key::Escape),
        "backspace" => Ok(Key::Backspace),
        "tab" => Ok(Key::Tab),
        "backtab" => Ok(Key::BackTab),
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
        "space" => Ok(Key::Char(' ')),
        _ => {
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

fn is_special_key(s: &str) -> bool {
    matches!(
        s.to_lowercase().as_str(),
        "enter"
            | "return"
            | "escape"
            | "esc"
            | "backspace"
            | "tab"
            | "backtab"
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
