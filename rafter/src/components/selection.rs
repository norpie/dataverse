//! Selection state management for components.
//!
//! This module provides shared selection types used by List, Tree, and Table components.
//! Selection uses string IDs for stability across item mutations.

use std::collections::HashSet;

/// Selection mode for components.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SelectionMode {
    /// No selection allowed
    #[default]
    None,
    /// Single item selection
    Single,
    /// Multiple items can be selected (Ctrl+click, Shift+range)
    Multiple,
}

/// ID-based selection state.
///
/// Used by List, Tree, and Table components for tracking selected items by their ID.
/// Using string IDs allows selection to remain stable when items are added/removed.
#[derive(Debug, Clone, Default)]
pub struct Selection {
    /// Currently selected IDs
    selected: HashSet<String>,
    /// Anchor for range selection (Shift+click starting point)
    anchor: Option<String>,
}

impl Selection {
    /// Create a new empty selection.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get all selected IDs (sorted for deterministic ordering).
    pub fn selected(&self) -> Vec<String> {
        let mut ids: Vec<_> = self.selected.iter().cloned().collect();
        ids.sort();
        ids
    }

    /// Check if an ID is selected.
    pub fn is_selected(&self, id: &str) -> bool {
        self.selected.contains(id)
    }

    /// Get the number of selected items.
    pub fn len(&self) -> usize {
        self.selected.len()
    }

    /// Check if nothing is selected.
    pub fn is_empty(&self) -> bool {
        self.selected.is_empty()
    }

    /// Get the anchor ID for range selection.
    pub fn anchor(&self) -> Option<&str> {
        self.anchor.as_deref()
    }

    /// Clear all selection.
    /// Returns the IDs that were deselected.
    pub fn clear(&mut self) -> Vec<String> {
        let removed: Vec<_> = self.selected.drain().collect();
        self.anchor = None;
        removed
    }

    /// Select a single ID (clears others).
    /// Returns (added, removed) IDs.
    pub fn select(&mut self, id: &str) -> (Vec<String>, Vec<String>) {
        let removed: Vec<_> = self.selected.iter().filter(|&i| i != id).cloned().collect();
        let was_selected = self.selected.contains(id);
        self.selected.clear();
        self.selected.insert(id.to_string());
        self.anchor = Some(id.to_string());
        let added = if was_selected {
            vec![]
        } else {
            vec![id.to_string()]
        };
        (added, removed)
    }

    /// Toggle selection of an ID (Ctrl+click behavior).
    /// Returns (added, removed) IDs.
    pub fn toggle(&mut self, id: &str) -> (Vec<String>, Vec<String>) {
        if self.selected.remove(id) {
            self.anchor = Some(id.to_string());
            (vec![], vec![id.to_string()])
        } else {
            self.selected.insert(id.to_string());
            self.anchor = Some(id.to_string());
            (vec![id.to_string()], vec![])
        }
    }

    /// Range select from anchor to target ID (Shift+click behavior).
    ///
    /// Requires the ordered list of all visible IDs to determine the range.
    /// If `extend` is false, clears selection outside the range first.
    ///
    /// Returns (added, removed) IDs.
    pub fn range_select(
        &mut self,
        target_id: &str,
        all_ids_ordered: &[String],
        extend: bool,
    ) -> (Vec<String>, Vec<String>) {
        let anchor_id = self.anchor.clone().unwrap_or_else(|| target_id.to_string());

        // Find positions of anchor and target in the ordered list
        let anchor_pos = all_ids_ordered.iter().position(|id| id == &anchor_id);
        let target_pos = all_ids_ordered.iter().position(|id| id == target_id);

        let (start, end) = match (anchor_pos, target_pos) {
            (Some(a), Some(t)) => {
                if a <= t {
                    (a, t)
                } else {
                    (t, a)
                }
            }
            // If anchor or target not found, just select the target
            _ => {
                return self.select(target_id);
            }
        };

        let mut added = Vec::new();
        let mut removed = Vec::new();

        // Get the IDs in the range
        let range_ids: HashSet<String> = all_ids_ordered[start..=end].iter().cloned().collect();

        if !extend {
            // Remove items outside the range
            removed = self
                .selected
                .iter()
                .filter(|id| !range_ids.contains(*id))
                .cloned()
                .collect();
            for id in &removed {
                self.selected.remove(id);
            }
        }

        // Add items in the range
        for id in &range_ids {
            if self.selected.insert(id.clone()) {
                added.push(id.clone());
            }
        }

        (added, removed)
    }

    /// Select all items from the provided list of IDs.
    /// Returns the IDs that were newly selected.
    pub fn select_all(&mut self, all_ids: &[String]) -> Vec<String> {
        let mut added = Vec::new();
        for id in all_ids {
            if self.selected.insert(id.clone()) {
                added.push(id.clone());
            }
        }
        added
    }
}
