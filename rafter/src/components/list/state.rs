//! List component state.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};

use crate::components::scrollbar::{
    ScrollbarConfig, ScrollbarDrag, ScrollbarGeometry, ScrollbarState,
};
use crate::components::selection::{Selection, SelectionMode};
use crate::components::traits::ScrollableComponent;

use super::item::ListItem;

/// Unique identifier for a List component instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ListId(usize);

impl ListId {
    fn new() -> Self {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        Self(COUNTER.fetch_add(1, Ordering::SeqCst))
    }
}

impl std::fmt::Display for ListId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "__list_{}", self.0)
    }
}

/// Internal state for the List component.
#[derive(Debug)]
pub(super) struct ListInner<T: ListItem> {
    /// The items in the list.
    pub items: Vec<T>,
    /// Selection state.
    pub selection: Selection,
    /// Selection mode.
    pub selection_mode: SelectionMode,
    /// Current cursor position (focused item).
    pub cursor: Option<usize>,
    /// Scroll offset in pixels/rows.
    pub scroll_offset: u16,
    /// Viewport height (set by renderer).
    pub viewport_height: u16,
    /// Scrollbar configuration.
    pub scrollbar: ScrollbarConfig,
    /// Vertical scrollbar geometry (set by renderer for hit testing).
    pub vertical_scrollbar: Option<ScrollbarGeometry>,
    /// Drag state for scrollbar interaction.
    pub drag: Option<ScrollbarDrag>,
}

impl<T: ListItem> Default for ListInner<T> {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            selection: Selection::new(),
            selection_mode: SelectionMode::None,
            cursor: None,
            scroll_offset: 0,
            viewport_height: 0,
            scrollbar: ScrollbarConfig::default(),
            vertical_scrollbar: None,
            drag: None,
        }
    }
}

/// A virtualized list component with selection support.
///
/// `List<T>` manages a collection of items with:
/// - Virtualized rendering (only visible items are rendered)
/// - Cursor navigation (keyboard focus on single item)
/// - Selection (single or multi-select)
/// - Activation (Enter/click to activate an item)
#[derive(Debug)]
pub struct List<T: ListItem> {
    /// Unique identifier.
    id: ListId,
    /// Internal state.
    pub(super) inner: Arc<RwLock<ListInner<T>>>,
    /// Dirty flag for re-render.
    pub(super) dirty: Arc<AtomicBool>,
}

impl<T: ListItem> List<T> {
    /// Create a new empty list.
    pub fn new() -> Self {
        Self {
            id: ListId::new(),
            inner: Arc::new(RwLock::new(ListInner::default())),
            dirty: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Create a list with initial items.
    pub fn with_items(items: Vec<T>) -> Self {
        Self {
            id: ListId::new(),
            inner: Arc::new(RwLock::new(ListInner {
                items,
                ..Default::default()
            })),
            dirty: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Create a list with selection mode.
    pub fn with_selection_mode(mode: SelectionMode) -> Self {
        Self {
            id: ListId::new(),
            inner: Arc::new(RwLock::new(ListInner {
                selection_mode: mode,
                ..Default::default()
            })),
            dirty: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Get the unique ID.
    pub fn id(&self) -> ListId {
        self.id
    }

    /// Get the ID as a string.
    pub fn id_string(&self) -> String {
        self.id.to_string()
    }

    // -------------------------------------------------------------------------
    // Item access
    // -------------------------------------------------------------------------

    /// Get the number of items.
    pub fn len(&self) -> usize {
        self.inner.read().map(|g| g.items.len()).unwrap_or(0)
    }

    /// Check if the list is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get an item by index.
    pub fn get(&self, index: usize) -> Option<T> {
        self.inner
            .read()
            .ok()
            .and_then(|g| g.items.get(index).cloned())
    }

    /// Get all items.
    pub fn items(&self) -> Vec<T> {
        self.inner
            .read()
            .map(|g| g.items.clone())
            .unwrap_or_default()
    }

    // -------------------------------------------------------------------------
    // Item mutation
    // -------------------------------------------------------------------------

    /// Set all items.
    pub fn set_items(&self, items: Vec<T>) {
        if let Ok(mut guard) = self.inner.write() {
            guard.items = items;
            // Clamp cursor
            if let Some(cursor) = guard.cursor
                && cursor >= guard.items.len()
            {
                guard.cursor = guard.items.len().checked_sub(1);
            }
            // Clear selection (items changed)
            guard.selection.clear();
            self.dirty.store(true, Ordering::SeqCst);
        }
    }

    /// Push an item to the end.
    pub fn push(&self, item: T) {
        if let Ok(mut guard) = self.inner.write() {
            guard.items.push(item);
            self.dirty.store(true, Ordering::SeqCst);
        }
    }

    /// Remove an item by index.
    pub fn remove(&self, index: usize) -> Option<T> {
        if let Ok(mut guard) = self.inner.write()
            && index < guard.items.len()
        {
            let item = guard.items.remove(index);
            guard.selection.on_item_removed(index);
            // Adjust cursor
            if let Some(cursor) = guard.cursor {
                if cursor == index {
                    // Stay at same position or move to last
                    guard.cursor = if guard.items.is_empty() {
                        None
                    } else {
                        Some(cursor.min(guard.items.len() - 1))
                    };
                } else if cursor > index {
                    guard.cursor = Some(cursor - 1);
                }
            }
            self.dirty.store(true, Ordering::SeqCst);
            return Some(item);
        }
        None
    }

    /// Clear all items.
    pub fn clear(&self) {
        if let Ok(mut guard) = self.inner.write() {
            guard.items.clear();
            guard.selection.clear();
            guard.cursor = None;
            guard.scroll_offset = 0;
            self.dirty.store(true, Ordering::SeqCst);
        }
    }

    /// Update items with a closure.
    pub fn update<F>(&self, f: F)
    where
        F: FnOnce(&mut Vec<T>),
    {
        if let Ok(mut guard) = self.inner.write() {
            f(&mut guard.items);
            // Clamp cursor
            if let Some(cursor) = guard.cursor
                && cursor >= guard.items.len()
            {
                guard.cursor = guard.items.len().checked_sub(1);
            }
            self.dirty.store(true, Ordering::SeqCst);
        }
    }

    // -------------------------------------------------------------------------
    // Cursor
    // -------------------------------------------------------------------------

    /// Get the current cursor position.
    pub fn cursor(&self) -> Option<usize> {
        self.inner.read().ok().and_then(|g| g.cursor)
    }

    /// Set the cursor position.
    pub fn set_cursor(&self, index: usize) -> Option<usize> {
        if let Ok(mut guard) = self.inner.write() {
            let previous = guard.cursor;
            // Only update and mark dirty if cursor actually changed
            if index < guard.items.len() && previous != Some(index) {
                guard.cursor = Some(index);
                self.dirty.store(true, Ordering::SeqCst);
            }
            return previous;
        }
        None
    }

    /// Move cursor up.
    pub fn cursor_up(&self) -> Option<(Option<usize>, usize)> {
        if let Ok(mut guard) = self.inner.write() {
            let previous = guard.cursor;
            if let Some(cursor) = guard.cursor {
                if cursor > 0 {
                    guard.cursor = Some(cursor - 1);
                    self.dirty.store(true, Ordering::SeqCst);
                    return Some((previous, cursor - 1));
                }
            } else if !guard.items.is_empty() {
                guard.cursor = Some(0);
                self.dirty.store(true, Ordering::SeqCst);
                return Some((None, 0));
            }
        }
        None
    }

    /// Move cursor down.
    pub fn cursor_down(&self) -> Option<(Option<usize>, usize)> {
        if let Ok(mut guard) = self.inner.write() {
            let previous = guard.cursor;
            let max_index = guard.items.len().saturating_sub(1);
            if let Some(cursor) = guard.cursor {
                if cursor < max_index {
                    guard.cursor = Some(cursor + 1);
                    self.dirty.store(true, Ordering::SeqCst);
                    return Some((previous, cursor + 1));
                }
            } else if !guard.items.is_empty() {
                guard.cursor = Some(0);
                self.dirty.store(true, Ordering::SeqCst);
                return Some((None, 0));
            }
        }
        None
    }

    /// Move cursor to first item.
    pub fn cursor_first(&self) -> Option<(Option<usize>, usize)> {
        if let Ok(mut guard) = self.inner.write()
            && !guard.items.is_empty()
        {
            let previous = guard.cursor;
            guard.cursor = Some(0);
            self.dirty.store(true, Ordering::SeqCst);
            return Some((previous, 0));
        }
        None
    }

    /// Move cursor to last item.
    pub fn cursor_last(&self) -> Option<(Option<usize>, usize)> {
        if let Ok(mut guard) = self.inner.write()
            && !guard.items.is_empty()
        {
            let previous = guard.cursor;
            let last = guard.items.len() - 1;
            guard.cursor = Some(last);
            self.dirty.store(true, Ordering::SeqCst);
            return Some((previous, last));
        }
        None
    }

    // -------------------------------------------------------------------------
    // Selection
    // -------------------------------------------------------------------------

    /// Get the selection mode.
    pub fn selection_mode(&self) -> SelectionMode {
        self.inner
            .read()
            .map(|g| g.selection_mode)
            .unwrap_or_default()
    }

    /// Set the selection mode.
    pub fn set_selection_mode(&self, mode: SelectionMode) {
        if let Ok(mut guard) = self.inner.write() {
            guard.selection_mode = mode;
            if mode == SelectionMode::None {
                guard.selection.clear();
            }
            self.dirty.store(true, Ordering::SeqCst);
        }
    }

    /// Get all selected indices.
    pub fn selected_indices(&self) -> Vec<usize> {
        self.inner
            .read()
            .map(|g| g.selection.selected())
            .unwrap_or_default()
    }

    /// Get all selected items.
    pub fn selected_items(&self) -> Vec<T> {
        self.inner
            .read()
            .map(|g| {
                g.selection
                    .selected()
                    .into_iter()
                    .filter_map(|i| g.items.get(i).cloned())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Check if an index is selected.
    pub fn is_selected(&self, index: usize) -> bool {
        self.inner
            .read()
            .map(|g| g.selection.is_selected(index))
            .unwrap_or(false)
    }

    /// Select a single item (clears other selection).
    pub fn select(&self, index: usize) -> (Vec<usize>, Vec<usize>) {
        if let Ok(mut guard) = self.inner.write()
            && guard.selection_mode != SelectionMode::None
            && index < guard.items.len()
        {
            let result = guard.selection.select(index);
            self.dirty.store(true, Ordering::SeqCst);
            return result;
        }
        (vec![], vec![])
    }

    /// Toggle selection of an item.
    pub fn toggle_select(&self, index: usize) -> (Vec<usize>, Vec<usize>) {
        if let Ok(mut guard) = self.inner.write()
            && guard.selection_mode == SelectionMode::Multiple
            && index < guard.items.len()
        {
            let result = guard.selection.toggle(index);
            self.dirty.store(true, Ordering::SeqCst);
            return result;
        }
        (vec![], vec![])
    }

    /// Select a range from anchor to index.
    pub fn range_select(&self, index: usize, extend: bool) -> (Vec<usize>, Vec<usize>) {
        if let Ok(mut guard) = self.inner.write()
            && guard.selection_mode == SelectionMode::Multiple
            && index < guard.items.len()
        {
            let result = guard.selection.range_select(index, extend);
            self.dirty.store(true, Ordering::SeqCst);
            return result;
        }
        (vec![], vec![])
    }

    /// Select all items.
    pub fn select_all(&self) -> Vec<usize> {
        if let Ok(mut guard) = self.inner.write()
            && guard.selection_mode == SelectionMode::Multiple
            && !guard.items.is_empty()
        {
            let max_index = guard.items.len() - 1;
            let result = guard.selection.select_all(max_index);
            self.dirty.store(true, Ordering::SeqCst);
            return result;
        }
        vec![]
    }

    /// Clear all selection.
    pub fn deselect_all(&self) -> Vec<usize> {
        if let Ok(mut guard) = self.inner.write() {
            let result = guard.selection.clear();
            self.dirty.store(true, Ordering::SeqCst);
            return result;
        }
        vec![]
    }

    // -------------------------------------------------------------------------
    // Scrolling
    // -------------------------------------------------------------------------

    /// Get the scroll offset.
    pub fn scroll_offset(&self) -> u16 {
        self.inner.read().map(|g| g.scroll_offset).unwrap_or(0)
    }

    /// Set the scroll offset.
    pub fn set_scroll_offset(&self, offset: u16) {
        if let Ok(mut guard) = self.inner.write() {
            let max_offset = Self::max_scroll_offset_inner(&guard);
            guard.scroll_offset = offset.min(max_offset);
            self.dirty.store(true, Ordering::SeqCst);
        }
    }

    /// Scroll to make an item visible.
    pub fn scroll_to_item(&self, index: usize) {
        if let Ok(mut guard) = self.inner.write() {
            if index >= guard.items.len() {
                return;
            }
            let item_height = T::HEIGHT;
            let item_top = index as u16 * item_height;
            let item_bottom = item_top + item_height;
            let viewport = guard.viewport_height;

            if viewport == 0 {
                return;
            }

            // If item is above viewport, scroll up
            if item_top < guard.scroll_offset {
                guard.scroll_offset = item_top;
                self.dirty.store(true, Ordering::SeqCst);
            }
            // If item is below viewport, scroll down
            else if item_bottom > guard.scroll_offset + viewport {
                guard.scroll_offset = item_bottom.saturating_sub(viewport);
                self.dirty.store(true, Ordering::SeqCst);
            }
        }
    }

    /// Scroll to cursor if it exists.
    pub fn scroll_to_cursor(&self) {
        if let Some(cursor) = self.cursor() {
            self.scroll_to_item(cursor);
        }
    }

    pub(super) fn max_scroll_offset_inner(guard: &ListInner<T>) -> u16 {
        let total_height = guard.items.len() as u16 * T::HEIGHT;
        total_height.saturating_sub(guard.viewport_height)
    }

    // -------------------------------------------------------------------------
    // Viewport (set by renderer)
    // -------------------------------------------------------------------------

    /// Set the viewport height (called by renderer).
    pub fn set_viewport_height(&self, height: u16) {
        if let Ok(mut guard) = self.inner.write() {
            guard.viewport_height = height;
            // Clamp scroll offset
            let max_offset = Self::max_scroll_offset_inner(&guard);
            if guard.scroll_offset > max_offset {
                guard.scroll_offset = max_offset;
            }
        }
    }

    /// Get the viewport height.
    pub fn viewport_height(&self) -> u16 {
        self.inner.read().map(|g| g.viewport_height).unwrap_or(0)
    }

    /// Get the visible item range.
    pub fn visible_range(&self) -> std::ops::Range<usize> {
        self.inner
            .read()
            .map(|g| self.visible_range_inner(&g))
            .unwrap_or(0..0)
    }

    /// Get total content height.
    pub fn total_height(&self) -> u16 {
        self.len() as u16 * T::HEIGHT
    }

    /// Check if the visible area is near the end of the list.
    ///
    /// Returns `true` if the last visible item is within `threshold` items
    /// of the end of the list. Useful for implementing infinite scroll / pagination.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Load more when within 10 items of the end
    /// if self.records.is_near_end(10) && !loading && has_more {
    ///     self.load_more(cx).await;
    /// }
    /// ```
    pub fn is_near_end(&self, threshold: usize) -> bool {
        self.inner
            .read()
            .map(|g| {
                if g.items.is_empty() {
                    return false;
                }
                let range = self.visible_range_inner(&g);
                let last_visible = range.end.saturating_sub(1);
                let total = g.items.len();
                last_visible + threshold >= total
            })
            .unwrap_or(false)
    }

    /// Internal helper to compute visible range.
    fn visible_range_inner(&self, g: &ListInner<T>) -> std::ops::Range<usize> {
        if g.items.is_empty() || g.viewport_height == 0 {
            return 0..0;
        }
        let item_height = T::HEIGHT;
        let start = (g.scroll_offset / item_height) as usize;
        let visible_count = g.viewport_height.div_ceil(item_height) as usize;
        let end = (start + visible_count + 1).min(g.items.len());
        start..end
    }

    // -------------------------------------------------------------------------
    // Dirty tracking
    // -------------------------------------------------------------------------

    /// Check if the list has changed.
    pub fn is_dirty(&self) -> bool {
        self.dirty.load(Ordering::SeqCst)
    }

    /// Clear the dirty flag.
    pub fn clear_dirty(&self) {
        self.dirty.store(false, Ordering::SeqCst);
    }
}

impl<T: ListItem> Clone for List<T> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            inner: Arc::clone(&self.inner),
            dirty: Arc::clone(&self.dirty),
        }
    }
}

impl<T: ListItem> Default for List<T> {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// ScrollbarState trait implementation
// =============================================================================

impl<T: ListItem> ScrollbarState for List<T> {
    fn scrollbar_config(&self) -> ScrollbarConfig {
        self.inner
            .read()
            .map(|guard| guard.scrollbar.clone())
            .unwrap_or_default()
    }

    fn set_scrollbar_config(&self, config: ScrollbarConfig) {
        if let Ok(mut guard) = self.inner.write() {
            guard.scrollbar = config;
            self.dirty.store(true, Ordering::SeqCst);
        }
    }

    fn scroll_offset_y(&self) -> u16 {
        self.scroll_offset()
    }

    fn scroll_to_y(&self, y: u16) {
        self.set_scroll_offset(y);
    }

    fn scroll_by(&self, _dx: i16, dy: i16) {
        if let Ok(mut guard) = self.inner.write() {
            let max_offset = Self::max_scroll_offset_inner(&guard);
            let new_y = (guard.scroll_offset as i32 + dy as i32).clamp(0, max_offset as i32) as u16;
            if new_y != guard.scroll_offset {
                guard.scroll_offset = new_y;
                self.dirty.store(true, Ordering::SeqCst);
            }
        }
    }

    fn scroll_to_top(&self) {
        if let Ok(mut guard) = self.inner.write()
            && guard.scroll_offset != 0
        {
            guard.scroll_offset = 0;
            self.dirty.store(true, Ordering::SeqCst);
        }
    }

    fn scroll_to_bottom(&self) {
        if let Ok(mut guard) = self.inner.write() {
            let max_offset = Self::max_scroll_offset_inner(&guard);
            if guard.scroll_offset != max_offset {
                guard.scroll_offset = max_offset;
                self.dirty.store(true, Ordering::SeqCst);
            }
        }
    }

    fn content_height(&self) -> u16 {
        self.total_height()
    }

    fn viewport_height(&self) -> u16 {
        List::viewport_height(self)
    }

    fn vertical_scrollbar(&self) -> Option<ScrollbarGeometry> {
        self.inner
            .read()
            .ok()
            .and_then(|guard| guard.vertical_scrollbar)
    }

    fn set_vertical_scrollbar(&self, geometry: Option<ScrollbarGeometry>) {
        if let Ok(mut guard) = self.inner.write() {
            guard.vertical_scrollbar = geometry;
        }
    }

    fn drag(&self) -> Option<ScrollbarDrag> {
        self.inner.read().map(|guard| guard.drag).unwrap_or(None)
    }

    fn set_drag(&self, drag: Option<ScrollbarDrag>) {
        if let Ok(mut guard) = self.inner.write() {
            guard.drag = drag;
        }
    }
}

// =============================================================================
// ScrollableComponent trait implementation
// =============================================================================

impl<T: ListItem> ScrollableComponent for List<T> {
    fn id_string(&self) -> String {
        self.id.to_string()
    }

    fn is_dirty(&self) -> bool {
        self.dirty.load(Ordering::SeqCst)
    }

    fn clear_dirty(&self) {
        self.dirty.store(false, Ordering::SeqCst);
    }
}
