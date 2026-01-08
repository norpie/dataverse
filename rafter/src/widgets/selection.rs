//! Shared selection types for list/tree/table widgets.

use std::collections::HashSet;
use std::hash::Hash;

/// Selection mode for list-like widgets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SelectionMode {
    /// No selection allowed.
    #[default]
    None,
    /// Single item selection (radio-button style).
    Single,
    /// Multiple items can be selected (checkbox style).
    Multi,
}

/// Tracks selected items by their keys.
#[derive(Debug, Clone)]
pub struct Selection<K: Clone + Eq + Hash> {
    pub mode: SelectionMode,
    pub selected: HashSet<K>,
}

impl<K: Clone + Eq + Hash> Default for Selection<K> {
    fn default() -> Self {
        Self::none()
    }
}

impl<K: Clone + Eq + Hash> Selection<K> {
    /// Create selection with no selection allowed.
    pub fn none() -> Self {
        Self {
            mode: SelectionMode::None,
            selected: HashSet::new(),
        }
    }

    /// Create single-selection mode.
    pub fn single() -> Self {
        Self {
            mode: SelectionMode::Single,
            selected: HashSet::new(),
        }
    }

    /// Create multi-selection mode.
    pub fn multi() -> Self {
        Self {
            mode: SelectionMode::Multi,
            selected: HashSet::new(),
        }
    }

    /// Toggle selection for a key. Returns true if selection changed.
    pub fn toggle(&mut self, key: K) -> bool {
        match self.mode {
            SelectionMode::None => false,
            SelectionMode::Single => {
                if self.selected.contains(&key) {
                    self.selected.clear();
                    true
                } else {
                    self.selected.clear();
                    self.selected.insert(key);
                    true
                }
            }
            SelectionMode::Multi => {
                if self.selected.contains(&key) {
                    self.selected.remove(&key);
                } else {
                    self.selected.insert(key);
                }
                true
            }
        }
    }

    /// Check if a key is selected.
    pub fn is_selected(&self, key: &K) -> bool {
        self.selected.contains(key)
    }

    /// Clear all selections.
    pub fn clear(&mut self) {
        self.selected.clear();
    }

    /// Get the single selected key (for Single mode).
    pub fn get_single(&self) -> Option<&K> {
        self.selected.iter().next()
    }

    /// Get all selected keys.
    pub fn get_all(&self) -> impl Iterator<Item = &K> {
        self.selected.iter()
    }
}
