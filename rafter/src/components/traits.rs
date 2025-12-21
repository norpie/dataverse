//! Shared traits for scrollable components.
//!
//! These traits define the common interface for components that support
//! scrolling and share the same state management patterns.

use crate::context::AppContext;
use crate::keybinds::{Key, KeyCombo};

use super::events::{ComponentEvent, ComponentEventKind, EventResult};
use super::scrollbar::ScrollbarState;
use super::selection::SelectionMode;

/// Trait for components that support scrollable content.
///
/// This trait combines identity management, dirty tracking, and scrollbar
/// functionality into a unified interface. Components implementing this
/// trait can use the shared scrollbar event handlers and rendering.
///
/// # Implementors
///
/// - `ScrollArea` - Generic scrollable container
/// - `List<T>` - Virtualized list with selection
/// - Future: `Tree<T>`, `Table<T>`
///
/// # Example
///
/// ```ignore
/// impl ScrollableComponent for MyComponent {
///     fn id_string(&self) -> String {
///         self.id.to_string()
///     }
///
///     fn is_dirty(&self) -> bool {
///         self.dirty.load(Ordering::SeqCst)
///     }
///
///     fn clear_dirty(&self) {
///         self.dirty.store(false, Ordering::SeqCst);
///     }
/// }
/// ```
pub trait ScrollableComponent: ScrollbarState {
    /// Get the unique ID as a string (for node binding).
    fn id_string(&self) -> String;

    /// Check if the component state has changed and needs re-render.
    fn is_dirty(&self) -> bool;

    /// Clear the dirty flag after rendering.
    fn clear_dirty(&self);
}

/// Trait for components that support cursor navigation and selection.
///
/// This trait provides a unified interface for List, Tree, and Table components,
/// enabling shared event handling logic for keyboard navigation and selection.
///
/// # Trait Hierarchy
///
/// ```text
/// ScrollbarState
///     └── ScrollableComponent
///             └── SelectableComponent
/// ```
///
/// # Provided Methods
///
/// The trait provides default implementations for:
/// - Event pushing helpers (`push_cursor_event`, `push_activate_event`, `push_selection_event`)
/// - Key handling (`handle_navigation_key`, `handle_selection_key`)
/// - Cursor movement (`handle_cursor_move`)
/// - Mouse hover (`handle_hover`)
/// - Viewport calculations (`index_from_viewport_y`)
pub trait SelectableComponent: ScrollableComponent {
    // =========================================================================
    // Required Methods - Cursor
    // =========================================================================

    /// Get the current cursor position (index).
    fn cursor(&self) -> Option<usize>;

    /// Set the cursor position. Returns the previous cursor position.
    fn set_cursor(&self, index: usize) -> Option<usize>;

    /// Get the ID of the item at the cursor position.
    fn cursor_id(&self) -> Option<String>;

    /// Move cursor up. Returns `(previous_cursor, new_cursor)` if moved.
    fn cursor_up(&self) -> Option<(Option<usize>, usize)>;

    /// Move cursor down. Returns `(previous_cursor, new_cursor)` if moved.
    fn cursor_down(&self) -> Option<(Option<usize>, usize)>;

    /// Move cursor to first item. Returns `(previous_cursor, new_cursor)` if moved.
    fn cursor_first(&self) -> Option<(Option<usize>, usize)>;

    /// Move cursor to last item. Returns `(previous_cursor, new_cursor)` if moved.
    fn cursor_last(&self) -> Option<(Option<usize>, usize)>;

    /// Scroll the viewport to make the cursor visible.
    fn scroll_to_cursor(&self);

    // =========================================================================
    // Required Methods - Selection
    // =========================================================================

    /// Get the selection mode.
    fn selection_mode(&self) -> SelectionMode;

    /// Get all selected IDs.
    fn selected_ids(&self) -> Vec<String>;

    /// Toggle selection of the item at the cursor.
    /// Returns (added IDs, removed IDs).
    fn toggle_select_at_cursor(&self) -> (Vec<String>, Vec<String>);

    /// Select all items. Returns the IDs that were newly selected.
    fn select_all(&self) -> Vec<String>;

    /// Clear all selection. Returns the IDs that were deselected.
    fn deselect_all(&self) -> Vec<String>;

    // =========================================================================
    // Required Methods - Sizing
    // =========================================================================

    /// Get the total number of items.
    fn item_count(&self) -> usize;

    /// Get the number of items that fit in the viewport.
    fn viewport_item_count(&self) -> usize;

    /// Get the height of a single item (in rows).
    fn item_height(&self) -> u16;

    // =========================================================================
    // Provided Methods - Event Pushing
    // =========================================================================

    /// Push a cursor move event to the context.
    fn push_cursor_event(&self, cx: &AppContext) {
        if let Some(id) = self.cursor_id() {
            cx.set_cursor(id, self.cursor());
            cx.push_event(ComponentEvent::new(
                ComponentEventKind::CursorMove,
                self.id_string(),
            ));
        }
    }

    /// Push an activate event to the context.
    fn push_activate_event(&self, cx: &AppContext) {
        if let Some(id) = self.cursor_id() {
            cx.set_activated(id, self.cursor());
            cx.push_event(ComponentEvent::new(
                ComponentEventKind::Activate,
                self.id_string(),
            ));
        }
    }

    /// Push a selection change event to the context.
    fn push_selection_event(&self, added: &[String], removed: &[String], cx: &AppContext) {
        if !added.is_empty() || !removed.is_empty() {
            cx.set_selected(self.selected_ids());
            cx.push_event(ComponentEvent::new(
                ComponentEventKind::SelectionChange,
                self.id_string(),
            ));
        }
    }

    // =========================================================================
    // Provided Methods - Cursor Movement
    // =========================================================================

    /// Handle cursor movement, scroll to cursor, and push event.
    /// Returns true if cursor was moved.
    fn handle_cursor_move(&self, new_cursor: usize, cx: &AppContext) -> bool {
        let previous = self.set_cursor(new_cursor);
        if previous != Some(new_cursor) {
            self.scroll_to_cursor();
            self.push_cursor_event(cx);
            true
        } else {
            false
        }
    }

    // =========================================================================
    // Provided Methods - Viewport Calculations
    // =========================================================================

    /// Calculate the item index from a viewport-relative y coordinate.
    ///
    /// Override this for components with headers (like Table) that need
    /// to account for a header row offset.
    fn index_from_viewport_y(&self, y: u16) -> Option<usize> {
        let scroll_offset = self.scroll_offset_y();
        let item_height = self.item_height();
        if item_height == 0 {
            return None;
        }
        let absolute_y = scroll_offset + y;
        let index = (absolute_y / item_height) as usize;
        if index < self.item_count() {
            Some(index)
        } else {
            None
        }
    }

    // =========================================================================
    // Provided Methods - Key Handling
    // =========================================================================

    /// Handle navigation key events (Up, Down, Home, End, PageUp, PageDown).
    ///
    /// Returns `Some(EventResult)` if the key was handled, `None` otherwise.
    fn handle_navigation_key(&self, key: &KeyCombo, cx: &AppContext) -> Option<EventResult> {
        match key.key {
            Key::Up => {
                if self.cursor_up().is_some() {
                    self.scroll_to_cursor();
                    self.push_cursor_event(cx);
                    Some(EventResult::Consumed)
                } else {
                    Some(EventResult::Ignored)
                }
            }
            Key::Down => {
                if self.cursor_down().is_some() {
                    self.scroll_to_cursor();
                    self.push_cursor_event(cx);
                    Some(EventResult::Consumed)
                } else {
                    Some(EventResult::Ignored)
                }
            }
            Key::Home => {
                if self.cursor_first().is_some() {
                    self.scroll_to_cursor();
                    self.push_cursor_event(cx);
                    Some(EventResult::Consumed)
                } else {
                    Some(EventResult::Ignored)
                }
            }
            Key::End => {
                if self.cursor_last().is_some() {
                    self.scroll_to_cursor();
                    self.push_cursor_event(cx);
                    Some(EventResult::Consumed)
                } else {
                    Some(EventResult::Ignored)
                }
            }
            Key::PageUp => {
                let page_size = self.viewport_item_count().max(1);
                let current = self.cursor().unwrap_or(0);
                let new_cursor = current.saturating_sub(page_size);
                if self.handle_cursor_move(new_cursor, cx) {
                    Some(EventResult::Consumed)
                } else {
                    Some(EventResult::Ignored)
                }
            }
            Key::PageDown => {
                let page_size = self.viewport_item_count().max(1);
                let current = self.cursor().unwrap_or(0);
                let max_index = self.item_count().saturating_sub(1);
                let new_cursor = (current + page_size).min(max_index);
                if self.handle_cursor_move(new_cursor, cx) {
                    Some(EventResult::Consumed)
                } else {
                    Some(EventResult::Ignored)
                }
            }
            _ => None,
        }
    }

    /// Handle selection key events (Space, Ctrl+A, Escape, Enter).
    ///
    /// Returns `Some(EventResult)` if the key was handled, `None` otherwise.
    fn handle_selection_key(&self, key: &KeyCombo, cx: &AppContext) -> Option<EventResult> {
        match key.key {
            Key::Char(' ') if self.selection_mode() == SelectionMode::Multiple => {
                let (added, removed) = self.toggle_select_at_cursor();
                self.push_selection_event(&added, &removed, cx);
                Some(EventResult::Consumed)
            }
            Key::Char('a') if key.modifiers.ctrl && self.selection_mode() == SelectionMode::Multiple => {
                let added = self.select_all();
                if !added.is_empty() {
                    self.push_selection_event(&added, &[], cx);
                }
                Some(EventResult::Consumed)
            }
            Key::Escape if self.selection_mode() != SelectionMode::None => {
                let removed = self.deselect_all();
                if !removed.is_empty() {
                    self.push_selection_event(&[], &removed, cx);
                }
                Some(EventResult::Consumed)
            }
            Key::Enter => {
                self.push_activate_event(cx);
                Some(EventResult::Consumed)
            }
            _ => None,
        }
    }

    // =========================================================================
    // Provided Methods - Mouse Handling
    // =========================================================================

    /// Handle hover event at the given viewport-relative y coordinate.
    ///
    /// Moves the cursor to the hovered item if valid.
    fn handle_hover(&self, y: u16, cx: &AppContext) -> EventResult {
        if let Some(index) = self.index_from_viewport_y(y) {
            self.handle_cursor_move(index, cx);
        }
        EventResult::Consumed
    }
}
