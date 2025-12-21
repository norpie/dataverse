//! Selection state management for components.
//!
//! This module provides shared selection types used by List, Tree, and Table components.

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

/// Index-based selection state.
///
/// Used by List and Table components for tracking selected items by their index.
/// Tree components use a different selection model based on node IDs.
#[derive(Debug, Clone, Default)]
pub struct Selection {
    /// Currently selected indices
    selected: HashSet<usize>,
    /// Anchor for range selection (Shift+click starting point)
    anchor: Option<usize>,
}

impl Selection {
    /// Create a new empty selection.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get all selected indices.
    pub fn selected(&self) -> Vec<usize> {
        let mut indices: Vec<_> = self.selected.iter().copied().collect();
        indices.sort_unstable();
        indices
    }

    /// Check if an index is selected.
    pub fn is_selected(&self, index: usize) -> bool {
        self.selected.contains(&index)
    }

    /// Get the number of selected items.
    pub fn len(&self) -> usize {
        self.selected.len()
    }

    /// Check if nothing is selected.
    pub fn is_empty(&self) -> bool {
        self.selected.is_empty()
    }

    /// Clear all selection.
    pub fn clear(&mut self) -> Vec<usize> {
        let removed: Vec<_> = self.selected.drain().collect();
        self.anchor = None;
        removed
    }

    /// Select a single index (clears others).
    pub fn select(&mut self, index: usize) -> (Vec<usize>, Vec<usize>) {
        let removed: Vec<_> = self
            .selected
            .iter()
            .copied()
            .filter(|&i| i != index)
            .collect();
        self.selected.clear();
        self.selected.insert(index);
        self.anchor = Some(index);
        let added = if removed.contains(&index) {
            vec![]
        } else {
            vec![index]
        };
        (added, removed)
    }

    /// Toggle selection of an index (Ctrl+click behavior).
    pub fn toggle(&mut self, index: usize) -> (Vec<usize>, Vec<usize>) {
        if self.selected.remove(&index) {
            self.anchor = Some(index);
            (vec![], vec![index])
        } else {
            self.selected.insert(index);
            self.anchor = Some(index);
            (vec![index], vec![])
        }
    }

    /// Range select from anchor to index (Shift+click behavior).
    /// If `extend` is false, clears existing selection first.
    pub fn range_select(&mut self, index: usize, extend: bool) -> (Vec<usize>, Vec<usize>) {
        let anchor = self.anchor.unwrap_or(index);
        let (start, end) = if anchor <= index {
            (anchor, index)
        } else {
            (index, anchor)
        };

        let mut added = Vec::new();
        let mut removed = Vec::new();

        if !extend {
            // Remove items outside the range
            removed = self
                .selected
                .iter()
                .copied()
                .filter(|&i| i < start || i > end)
                .collect();
            for &i in &removed {
                self.selected.remove(&i);
            }
        }

        // Add items in the range
        for i in start..=end {
            if self.selected.insert(i) {
                added.push(i);
            }
        }

        (added, removed)
    }

    /// Select all items up to max_index.
    pub fn select_all(&mut self, max_index: usize) -> Vec<usize> {
        let mut added = Vec::new();
        for i in 0..=max_index {
            if self.selected.insert(i) {
                added.push(i);
            }
        }
        added
    }

    /// Handle item removal - shift indices down.
    pub fn on_item_removed(&mut self, removed_index: usize) {
        // Remove the deleted index
        self.selected.remove(&removed_index);

        // Shift all indices above it down
        let shifted: HashSet<usize> = self
            .selected
            .iter()
            .map(|&i| if i > removed_index { i - 1 } else { i })
            .collect();
        self.selected = shifted;

        // Adjust anchor
        if let Some(anchor) = self.anchor {
            if anchor == removed_index {
                self.anchor = None;
            } else if anchor > removed_index {
                self.anchor = Some(anchor - 1);
            }
        }
    }
}
