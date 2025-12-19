//! Focus management system.

/// Unique identifier for a focusable element
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FocusId(pub String);

impl FocusId {
    /// Create a new focus ID
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

impl From<&str> for FocusId {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for FocusId {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

/// Focus state manager
#[derive(Debug, Default)]
pub struct FocusState {
    /// Currently focused element ID
    current: Option<FocusId>,
    /// List of focusable elements in tab order
    focusable_ids: Vec<FocusId>,
    /// Whether focus needs to be updated
    focus_changed: bool,
}

impl FocusState {
    /// Create a new focus state
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the currently focused element
    pub fn current(&self) -> Option<&FocusId> {
        self.current.as_ref()
    }

    /// Check if an element is focused
    pub fn is_focused(&self, id: &str) -> bool {
        self.current.as_ref().is_some_and(|current| current.0 == id)
    }

    /// Set focus to a specific element
    pub fn set_focus(&mut self, id: impl Into<FocusId>) {
        self.current = Some(id.into());
        self.focus_changed = true;
    }

    /// Clear focus
    pub fn clear_focus(&mut self) {
        self.current = None;
        self.focus_changed = true;
    }

    /// Update the list of focusable elements (called during render)
    pub fn set_focusable_ids(&mut self, ids: Vec<FocusId>) {
        self.focusable_ids = ids;

        // If current focus is no longer valid, clear it
        if let Some(ref current) = self.current
            && !self.focusable_ids.contains(current)
        {
            self.current = None;
        }

        // If no focus and there are focusable elements, focus the first one
        if self.current.is_none() && !self.focusable_ids.is_empty() {
            self.current = Some(self.focusable_ids[0].clone());
        }
    }

    /// Move focus to the next element
    pub fn focus_next(&mut self) {
        if self.focusable_ids.is_empty() {
            return;
        }

        let current_idx = self
            .current
            .as_ref()
            .and_then(|c| self.focusable_ids.iter().position(|id| id == c));

        let next_idx = match current_idx {
            Some(idx) => (idx + 1) % self.focusable_ids.len(),
            None => 0,
        };

        self.current = Some(self.focusable_ids[next_idx].clone());
        self.focus_changed = true;
    }

    /// Move focus to the previous element
    pub fn focus_prev(&mut self) {
        if self.focusable_ids.is_empty() {
            return;
        }

        let current_idx = self
            .current
            .as_ref()
            .and_then(|c| self.focusable_ids.iter().position(|id| id == c));

        let prev_idx = match current_idx {
            Some(idx) => {
                if idx == 0 {
                    self.focusable_ids.len() - 1
                } else {
                    idx - 1
                }
            }
            None => 0,
        };

        self.current = Some(self.focusable_ids[prev_idx].clone());
        self.focus_changed = true;
    }

    /// Check if focus changed since last check
    pub fn take_focus_changed(&mut self) -> bool {
        let changed = self.focus_changed;
        self.focus_changed = false;
        changed
    }
}
