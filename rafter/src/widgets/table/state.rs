//! Table widget state.

use std::ops::Range;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};

use crate::widgets::scrollbar::{
    ScrollbarConfig, ScrollbarDrag, ScrollbarGeometry, ScrollbarState,
};
use crate::widgets::selection::{Selection, SelectionMode};
use crate::widgets::traits::{ScrollableWidget, SelectableWidget};

use super::item::{Column, TableRow};

/// Unique identifier for a Table widget instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TableId(usize);

impl TableId {
    fn new() -> Self {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        Self(COUNTER.fetch_add(1, Ordering::SeqCst))
    }
}

impl std::fmt::Display for TableId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "__table_{}", self.0)
    }
}

/// Internal state for the Table widget.
#[derive(Debug)]
pub(super) struct TableInner<T: TableRow> {
    /// Column definitions.
    pub columns: Vec<Column>,
    /// The rows in the table.
    pub rows: Vec<T>,
    /// Selection state (by row ID).
    pub selection: Selection,
    /// Selection mode.
    pub selection_mode: SelectionMode,
    /// Current cursor position (focused row).
    pub cursor: Option<usize>,
    /// Vertical scroll offset in rows.
    pub scroll_offset_y: u16,
    /// Horizontal scroll offset in terminal columns (character position).
    pub scroll_offset_x: u16,
    /// Viewport height (including header row).
    pub viewport_height: u16,
    /// Viewport width (available for columns).
    pub viewport_width: u16,
    /// Current sort state (column index, ascending).
    pub sort: Option<(usize, bool)>,
    /// Scrollbar configuration.
    pub scrollbar: ScrollbarConfig,
    /// Vertical scrollbar geometry (set by renderer for hit testing).
    pub vertical_scrollbar: Option<ScrollbarGeometry>,
    /// Horizontal scrollbar geometry.
    pub horizontal_scrollbar: Option<ScrollbarGeometry>,
    /// Drag state for scrollbar interaction.
    pub drag: Option<ScrollbarDrag>,
    /// Cached column x-positions for quick lookup.
    /// column_positions[i] = x-coordinate where column i starts.
    pub column_positions: Vec<u16>,
}

impl<T: TableRow> TableInner<T> {
    fn new(columns: Vec<Column>) -> Self {
        let column_positions = Self::calculate_column_positions(&columns);
        Self {
            columns,
            rows: Vec::new(),
            selection: Selection::new(),
            selection_mode: SelectionMode::None,
            cursor: None,
            scroll_offset_y: 0,
            scroll_offset_x: 0,
            viewport_height: 0,
            viewport_width: 0,
            sort: None,
            scrollbar: ScrollbarConfig::default(),
            vertical_scrollbar: None,
            horizontal_scrollbar: None,
            drag: None,
            column_positions,
        }
    }

    /// Calculate x-positions for each column.
    fn calculate_column_positions(columns: &[Column]) -> Vec<u16> {
        let mut positions = Vec::with_capacity(columns.len());
        let mut x = 0;
        for col in columns {
            positions.push(x);
            x += col.width;
        }
        positions
    }

    /// Get total content width (sum of all column widths).
    fn total_width(&self) -> u16 {
        self.columns.iter().map(|c| c.width).sum()
    }
}

/// A virtualized table widget with column support and selection.
///
/// `Table<T>` manages a collection of rows with:
/// - Column-based layout with headers
/// - Bi-directional virtualization (only visible rows and columns rendered)
/// - Cursor navigation (keyboard focus on single row)
/// - Selection (single or multi-select)
/// - Sortable columns (app-controlled)
/// - Horizontal and vertical scrolling
#[derive(Debug)]
pub struct Table<T: TableRow> {
    /// Unique identifier.
    id: TableId,
    /// Internal state.
    pub(super) inner: Arc<RwLock<TableInner<T>>>,
    /// Dirty flag for re-render.
    pub(super) dirty: Arc<AtomicBool>,
}

impl<T: TableRow> Table<T> {
    /// Create a new table with column definitions.
    pub fn new(columns: Vec<Column>) -> Self {
        Self {
            id: TableId::new(),
            inner: Arc::new(RwLock::new(TableInner::new(columns))),
            dirty: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Create a table with initial rows.
    pub fn with_rows(columns: Vec<Column>, rows: Vec<T>) -> Self {
        let mut inner = TableInner::new(columns);
        inner.rows = rows;
        Self {
            id: TableId::new(),
            inner: Arc::new(RwLock::new(inner)),
            dirty: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Set the selection mode.
    pub fn with_selection_mode(self, mode: SelectionMode) -> Self {
        if let Ok(mut guard) = self.inner.write() {
            guard.selection_mode = mode;
        }
        self
    }

    /// Get the unique ID.
    pub fn id(&self) -> TableId {
        self.id
    }

    /// Get the ID as a string.
    pub fn id_string(&self) -> String {
        self.id.to_string()
    }

    // -------------------------------------------------------------------------
    // Column access
    // -------------------------------------------------------------------------

    /// Get the column definitions.
    pub fn columns(&self) -> Vec<Column> {
        self.inner
            .read()
            .map(|g| g.columns.clone())
            .unwrap_or_default()
    }

    /// Set the column definitions.
    pub fn set_columns(&self, columns: Vec<Column>) {
        if let Ok(mut guard) = self.inner.write() {
            guard.column_positions = TableInner::<T>::calculate_column_positions(&columns);
            guard.columns = columns;
            // Reset horizontal scroll
            guard.scroll_offset_x = 0;
            self.dirty.store(true, Ordering::SeqCst);
        }
    }

    /// Get the number of columns.
    pub fn column_count(&self) -> usize {
        self.inner.read().map(|g| g.columns.len()).unwrap_or(0)
    }

    /// Get total content width (sum of all column widths).
    pub fn total_width(&self) -> u16 {
        self.inner.read().map(|g| g.total_width()).unwrap_or(0)
    }

    /// Find which column is at a given x-coordinate.
    fn column_at_x(positions: &[u16], columns: &[Column], x: u16) -> usize {
        if positions.is_empty() {
            return 0;
        }
        // Binary search for the column
        match positions.binary_search(&x) {
            Ok(i) => i,
            Err(i) => i.saturating_sub(1).min(columns.len().saturating_sub(1)),
        }
    }

    // -------------------------------------------------------------------------
    // Row access
    // -------------------------------------------------------------------------

    /// Get the number of rows.
    pub fn len(&self) -> usize {
        self.inner.read().map(|g| g.rows.len()).unwrap_or(0)
    }

    /// Check if the table is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get a row by index.
    pub fn row(&self, index: usize) -> Option<T> {
        self.inner
            .read()
            .ok()
            .and_then(|g| g.rows.get(index).cloned())
    }

    /// Get all rows.
    pub fn rows(&self) -> Vec<T> {
        self.inner
            .read()
            .map(|g| g.rows.clone())
            .unwrap_or_default()
    }

    /// Find a row by ID.
    pub fn find_row(&self, id: &str) -> Option<(usize, T)> {
        self.inner.read().ok().and_then(|g| {
            g.rows
                .iter()
                .enumerate()
                .find(|(_, row)| row.id() == id)
                .map(|(i, row)| (i, row.clone()))
        })
    }

    /// Get all row IDs in order.
    fn all_ids(guard: &TableInner<T>) -> Vec<String> {
        guard.rows.iter().map(|row| row.id()).collect()
    }

    // -------------------------------------------------------------------------
    // Row mutation
    // -------------------------------------------------------------------------

    /// Set all rows.
    pub fn set_rows(&self, rows: Vec<T>) {
        if let Ok(mut guard) = self.inner.write() {
            guard.rows = rows;
            // Clamp cursor
            if let Some(cursor) = guard.cursor
                && cursor >= guard.rows.len()
            {
                guard.cursor = guard.rows.len().checked_sub(1);
            }
            // Clear selection (rows changed)
            guard.selection.clear();
            self.dirty.store(true, Ordering::SeqCst);
        }
    }

    /// Clear all rows.
    pub fn clear(&self) {
        if let Ok(mut guard) = self.inner.write() {
            guard.rows.clear();
            guard.selection.clear();
            guard.cursor = None;
            guard.scroll_offset_y = 0;
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

    /// Get the row at the cursor position.
    pub fn cursor_row(&self) -> Option<T> {
        self.inner
            .read()
            .ok()
            .and_then(|g| g.cursor.and_then(|c| g.rows.get(c).cloned()))
    }

    /// Get the ID of the row at the cursor position.
    pub fn cursor_id(&self) -> Option<String> {
        self.inner
            .read()
            .ok()
            .and_then(|g| g.cursor.and_then(|c| g.rows.get(c).map(|r| r.id())))
    }

    /// Set the cursor position.
    pub fn set_cursor(&self, index: usize) -> Option<usize> {
        if let Ok(mut guard) = self.inner.write() {
            let previous = guard.cursor;
            if index < guard.rows.len() && previous != Some(index) {
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
            } else if !guard.rows.is_empty() {
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
            let max_index = guard.rows.len().saturating_sub(1);
            if let Some(cursor) = guard.cursor {
                if cursor < max_index {
                    guard.cursor = Some(cursor + 1);
                    self.dirty.store(true, Ordering::SeqCst);
                    return Some((previous, cursor + 1));
                }
            } else if !guard.rows.is_empty() {
                guard.cursor = Some(0);
                self.dirty.store(true, Ordering::SeqCst);
                return Some((None, 0));
            }
        }
        None
    }

    /// Move cursor to first row.
    pub fn cursor_first(&self) -> Option<(Option<usize>, usize)> {
        if let Ok(mut guard) = self.inner.write()
            && !guard.rows.is_empty()
        {
            let previous = guard.cursor;
            guard.cursor = Some(0);
            self.dirty.store(true, Ordering::SeqCst);
            return Some((previous, 0));
        }
        None
    }

    /// Move cursor to last row.
    pub fn cursor_last(&self) -> Option<(Option<usize>, usize)> {
        if let Ok(mut guard) = self.inner.write()
            && !guard.rows.is_empty()
        {
            let previous = guard.cursor;
            let last = guard.rows.len() - 1;
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

    /// Get all selected IDs.
    pub fn selected_ids(&self) -> Vec<String> {
        self.inner
            .read()
            .map(|g| g.selection.selected())
            .unwrap_or_default()
    }

    /// Get all selected rows.
    pub fn selected_rows(&self) -> Vec<T> {
        self.inner
            .read()
            .map(|g| {
                let selected = g.selection.selected();
                g.rows
                    .iter()
                    .filter(|row| selected.contains(&row.id()))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Check if a row is selected by ID.
    pub fn is_selected(&self, id: &str) -> bool {
        self.inner
            .read()
            .map(|g| g.selection.is_selected(id))
            .unwrap_or(false)
    }

    /// Check if a row at index is selected.
    pub fn is_selected_at(&self, index: usize) -> bool {
        self.inner
            .read()
            .map(|g| {
                g.rows
                    .get(index)
                    .map(|row| g.selection.is_selected(&row.id()))
                    .unwrap_or(false)
            })
            .unwrap_or(false)
    }

    /// Select a single row by ID (clears other selection).
    /// Returns (added IDs, removed IDs).
    pub fn select(&self, id: &str) -> (Vec<String>, Vec<String>) {
        if let Ok(mut guard) = self.inner.write()
            && guard.selection_mode != SelectionMode::None
        {
            let result = guard.selection.select(id);
            self.dirty.store(true, Ordering::SeqCst);
            return result;
        }
        (vec![], vec![])
    }

    /// Toggle selection of a row by ID.
    /// Returns (added IDs, removed IDs).
    pub fn toggle_select(&self, id: &str) -> (Vec<String>, Vec<String>) {
        if let Ok(mut guard) = self.inner.write()
            && guard.selection_mode == SelectionMode::Multiple
        {
            let result = guard.selection.toggle(id);
            self.dirty.store(true, Ordering::SeqCst);
            return result;
        }
        (vec![], vec![])
    }

    /// Toggle selection of the row at the cursor.
    /// Returns (added IDs, removed IDs).
    pub fn toggle_select_at_cursor(&self) -> (Vec<String>, Vec<String>) {
        if let Some(id) = self.cursor_id() {
            self.toggle_select(&id)
        } else {
            (vec![], vec![])
        }
    }

    /// Select a range from anchor to the row with given ID.
    /// Returns (added IDs, removed IDs).
    pub fn range_select(&self, id: &str, extend: bool) -> (Vec<String>, Vec<String>) {
        if let Ok(mut guard) = self.inner.write()
            && guard.selection_mode == SelectionMode::Multiple
        {
            let all_ids = Self::all_ids(&guard);
            let result = guard.selection.range_select(id, &all_ids, extend);
            self.dirty.store(true, Ordering::SeqCst);
            return result;
        }
        (vec![], vec![])
    }

    /// Select all rows.
    /// Returns the IDs that were newly selected.
    pub fn select_all(&self) -> Vec<String> {
        if let Ok(mut guard) = self.inner.write()
            && guard.selection_mode == SelectionMode::Multiple
            && !guard.rows.is_empty()
        {
            let all_ids = Self::all_ids(&guard);
            let result = guard.selection.select_all(&all_ids);
            self.dirty.store(true, Ordering::SeqCst);
            return result;
        }
        vec![]
    }

    /// Clear all selection.
    /// Returns the IDs that were deselected.
    pub fn deselect_all(&self) -> Vec<String> {
        if let Ok(mut guard) = self.inner.write() {
            let result = guard.selection.clear();
            self.dirty.store(true, Ordering::SeqCst);
            return result;
        }
        vec![]
    }

    // -------------------------------------------------------------------------
    // Sorting
    // -------------------------------------------------------------------------

    /// Get current sort state.
    pub fn sort(&self) -> Option<(usize, bool)> {
        self.inner.read().ok().and_then(|g| g.sort)
    }

    /// Set sort by column index and direction.
    ///
    /// This DOES NOT sort the rows - it just stores the sort state.
    /// The app is responsible for sorting the data and calling `set_rows()`.
    pub fn set_sort(&self, column_index: usize, ascending: bool) {
        if let Ok(mut guard) = self.inner.write()
            && column_index < guard.columns.len()
            && guard.columns[column_index].sortable
        {
            guard.sort = Some((column_index, ascending));
            self.dirty.store(true, Ordering::SeqCst);
        }
    }

    /// Toggle sort for a column.
    ///
    /// If the column is already sorted, toggles the direction.
    /// If sorting a different column, sorts ascending.
    /// Returns the new sort state.
    pub fn toggle_sort(&self, column_index: usize) -> Option<(usize, bool)> {
        if let Ok(mut guard) = self.inner.write()
            && column_index < guard.columns.len()
            && guard.columns[column_index].sortable
        {
            let new_sort = match guard.sort {
                Some((idx, asc)) if idx == column_index => (column_index, !asc),
                _ => (column_index, true), // Default to ascending
            };
            guard.sort = Some(new_sort);
            self.dirty.store(true, Ordering::SeqCst);
            return Some(new_sort);
        }
        None
    }

    /// Clear sort state.
    pub fn clear_sort(&self) {
        if let Ok(mut guard) = self.inner.write() {
            guard.sort = None;
            self.dirty.store(true, Ordering::SeqCst);
        }
    }

    // -------------------------------------------------------------------------
    // Vertical Scrolling
    // -------------------------------------------------------------------------

    /// Get the vertical scroll offset (in rows).
    pub fn scroll_offset_y(&self) -> u16 {
        self.inner.read().map(|g| g.scroll_offset_y).unwrap_or(0)
    }

    /// Set the vertical scroll offset.
    pub fn set_scroll_offset_y(&self, offset: u16) {
        if let Ok(mut guard) = self.inner.write() {
            let max_offset = Self::max_scroll_offset_y_inner(&guard);
            guard.scroll_offset_y = offset.min(max_offset);
            self.dirty.store(true, Ordering::SeqCst);
        }
    }

    /// Scroll to make a row visible.
    pub fn scroll_to_row(&self, index: usize) {
        if let Ok(mut guard) = self.inner.write() {
            if index >= guard.rows.len() {
                return;
            }
            let row_height = T::HEIGHT;
            let row_top = index as u16 * row_height;
            let row_bottom = row_top + row_height;
            // Data viewport excludes header row
            let data_viewport = guard.viewport_height.saturating_sub(1);

            if data_viewport == 0 {
                return;
            }

            // If row is above viewport, scroll up
            if row_top < guard.scroll_offset_y {
                guard.scroll_offset_y = row_top;
                self.dirty.store(true, Ordering::SeqCst);
            }
            // If row is below viewport, scroll down
            else if row_bottom > guard.scroll_offset_y + data_viewport {
                guard.scroll_offset_y = row_bottom.saturating_sub(data_viewport);
                self.dirty.store(true, Ordering::SeqCst);
            }
        }
    }

    /// Scroll to cursor if it exists.
    pub fn scroll_to_cursor(&self) {
        if let Some(cursor) = self.cursor() {
            self.scroll_to_row(cursor);
        }
    }

    /// Get total content height (rows only, not header).
    pub fn total_height(&self) -> u16 {
        self.len() as u16 * T::HEIGHT
    }

    /// Get the data viewport height (excluding header row).
    pub fn data_viewport_height(&self) -> u16 {
        self.inner
            .read()
            .map(|g| g.viewport_height.saturating_sub(1))
            .unwrap_or(0)
    }

    /// Get the row height (from the row type).
    pub fn item_height(&self) -> u16 {
        T::HEIGHT
    }

    /// Get the number of rows that fit in the data viewport.
    pub fn viewport_item_count(&self) -> usize {
        (self.data_viewport_height() / T::HEIGHT) as usize
    }

    /// Check if vertical scrollbar is needed.
    pub fn needs_vertical_scrollbar(&self) -> bool {
        self.total_height() > self.data_viewport_height()
    }

    fn max_scroll_offset_y_inner(guard: &TableInner<T>) -> u16 {
        let total_height = guard.rows.len() as u16 * T::HEIGHT;
        let data_viewport = guard.viewport_height.saturating_sub(1); // Exclude header
        total_height.saturating_sub(data_viewport)
    }

    // -------------------------------------------------------------------------
    // Horizontal Scrolling
    // -------------------------------------------------------------------------

    /// Get the horizontal scroll offset (in terminal columns).
    pub fn scroll_offset_x(&self) -> u16 {
        self.inner.read().map(|g| g.scroll_offset_x).unwrap_or(0)
    }

    /// Set the horizontal scroll offset.
    pub fn set_scroll_offset_x(&self, offset: u16) {
        if let Ok(mut guard) = self.inner.write() {
            let max_offset = Self::max_scroll_offset_x_inner(&guard);
            guard.scroll_offset_x = offset.min(max_offset);
            self.dirty.store(true, Ordering::SeqCst);
        }
    }

    /// Scroll horizontally by N columns (can be negative).
    pub fn scroll_x_by(&self, delta: i16) {
        if let Ok(mut guard) = self.inner.write() {
            let max_offset = Self::max_scroll_offset_x_inner(&guard);
            let new_x =
                (guard.scroll_offset_x as i32 + delta as i32).clamp(0, max_offset as i32) as u16;
            if new_x != guard.scroll_offset_x {
                guard.scroll_offset_x = new_x;
                self.dirty.store(true, Ordering::SeqCst);
            }
        }
    }

    /// Scroll to make a specific column visible.
    pub fn scroll_to_column(&self, column_index: usize) {
        if let Ok(mut guard) = self.inner.write() {
            if column_index >= guard.columns.len() {
                return;
            }
            let col_left = guard.column_positions[column_index];
            let col_right = col_left + guard.columns[column_index].width;
            let viewport_width = guard.viewport_width;

            if viewport_width == 0 {
                return;
            }

            // If column is left of viewport, scroll left
            if col_left < guard.scroll_offset_x {
                guard.scroll_offset_x = col_left;
                self.dirty.store(true, Ordering::SeqCst);
            }
            // If column is right of viewport, scroll right
            else if col_right > guard.scroll_offset_x + viewport_width {
                guard.scroll_offset_x = col_right.saturating_sub(viewport_width);
                self.dirty.store(true, Ordering::SeqCst);
            }
        }
    }

    /// Check if horizontal scrollbar is needed.
    pub fn needs_horizontal_scrollbar(&self) -> bool {
        self.inner
            .read()
            .map(|g| g.total_width() > g.viewport_width)
            .unwrap_or(false)
    }

    fn max_scroll_offset_x_inner(guard: &TableInner<T>) -> u16 {
        guard.total_width().saturating_sub(guard.viewport_width)
    }

    /// Calculate which columns are visible (partially or fully).
    /// Returns range of column indices overlapping the viewport.
    pub fn visible_column_range(&self) -> Range<usize> {
        self.inner
            .read()
            .map(|g| Self::visible_column_range_inner(&g))
            .unwrap_or(0..0)
    }

    fn visible_column_range_inner(g: &TableInner<T>) -> Range<usize> {
        if g.columns.is_empty() || g.viewport_width == 0 {
            return 0..0;
        }

        let scroll_x = g.scroll_offset_x;
        let viewport_end = scroll_x + g.viewport_width;

        // Find first column that overlaps viewport
        let start = Self::column_at_x(&g.column_positions, &g.columns, scroll_x);

        // Find last column that overlaps viewport
        let mut end = start;
        for i in start..g.columns.len() {
            let col_left = g.column_positions[i];
            if col_left >= viewport_end {
                break;
            }
            end = i + 1;
        }

        start..end
    }

    // -------------------------------------------------------------------------
    // Viewport (set by renderer)
    // -------------------------------------------------------------------------

    /// Set the viewport height (called by renderer).
    pub fn set_viewport_height(&self, height: u16) {
        if let Ok(mut guard) = self.inner.write() {
            guard.viewport_height = height;
            // Clamp scroll offset
            let max_offset = Self::max_scroll_offset_y_inner(&guard);
            if guard.scroll_offset_y > max_offset {
                guard.scroll_offset_y = max_offset;
            }
        }
    }

    /// Get the viewport height.
    pub fn viewport_height(&self) -> u16 {
        self.inner.read().map(|g| g.viewport_height).unwrap_or(0)
    }

    /// Set the viewport width (called by renderer).
    pub fn set_viewport_width(&self, width: u16) {
        if let Ok(mut guard) = self.inner.write() {
            guard.viewport_width = width;
            // Clamp scroll offset
            let max_offset = Self::max_scroll_offset_x_inner(&guard);
            if guard.scroll_offset_x > max_offset {
                guard.scroll_offset_x = max_offset;
            }
        }
    }

    /// Get the viewport width.
    pub fn viewport_width(&self) -> u16 {
        self.inner.read().map(|g| g.viewport_width).unwrap_or(0)
    }

    /// Get the visible row range.
    pub fn visible_row_range(&self) -> Range<usize> {
        self.inner
            .read()
            .map(|g| Self::visible_row_range_inner(&g))
            .unwrap_or(0..0)
    }

    fn visible_row_range_inner(g: &TableInner<T>) -> Range<usize> {
        if g.rows.is_empty() || g.viewport_height <= 1 {
            return 0..0;
        }
        let row_height = T::HEIGHT;
        let data_viewport = g.viewport_height.saturating_sub(1); // Exclude header
        let start = (g.scroll_offset_y / row_height) as usize;
        let visible_count = data_viewport.div_ceil(row_height) as usize;
        let end = (start + visible_count + 1).min(g.rows.len());
        start..end
    }

    // -------------------------------------------------------------------------
    // Dirty tracking
    // -------------------------------------------------------------------------

    /// Check if the table has changed.
    pub fn is_dirty(&self) -> bool {
        self.dirty.load(Ordering::SeqCst)
    }

    /// Clear the dirty flag.
    pub fn clear_dirty(&self) {
        self.dirty.store(false, Ordering::SeqCst);
    }
}

impl<T: TableRow> Clone for Table<T> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            inner: Arc::clone(&self.inner),
            dirty: Arc::clone(&self.dirty),
        }
    }
}

impl<T: TableRow> Default for Table<T> {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

// =============================================================================
// ScrollbarState trait implementation
// =============================================================================

impl<T: TableRow> ScrollbarState for Table<T> {
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
        Table::scroll_offset_y(self)
    }

    fn scroll_to_y(&self, y: u16) {
        self.set_scroll_offset_y(y);
    }

    fn scroll_by(&self, dx: i16, dy: i16) {
        if dy != 0
            && let Ok(mut guard) = self.inner.write()
        {
            let max_offset = Self::max_scroll_offset_y_inner(&guard);
            let new_y =
                (guard.scroll_offset_y as i32 + dy as i32).clamp(0, max_offset as i32) as u16;
            if new_y != guard.scroll_offset_y {
                guard.scroll_offset_y = new_y;
                self.dirty.store(true, Ordering::SeqCst);
            }
        }
        if dx != 0 {
            self.scroll_x_by(dx);
        }
    }

    fn scroll_to_top(&self) {
        if let Ok(mut guard) = self.inner.write()
            && guard.scroll_offset_y != 0
        {
            guard.scroll_offset_y = 0;
            self.dirty.store(true, Ordering::SeqCst);
        }
    }

    fn scroll_to_bottom(&self) {
        if let Ok(mut guard) = self.inner.write() {
            let max_offset = Self::max_scroll_offset_y_inner(&guard);
            if guard.scroll_offset_y != max_offset {
                guard.scroll_offset_y = max_offset;
                self.dirty.store(true, Ordering::SeqCst);
            }
        }
    }

    fn content_height(&self) -> u16 {
        self.total_height()
    }

    fn viewport_height(&self) -> u16 {
        self.data_viewport_height()
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
// ScrollableWidget trait implementation
// =============================================================================

impl<T: TableRow> ScrollableWidget for Table<T> {
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

// =============================================================================
// SelectableWidget trait implementation
// =============================================================================

impl<T: TableRow> SelectableWidget for Table<T> {
    fn cursor(&self) -> Option<usize> {
        Table::cursor(self)
    }

    fn set_cursor(&self, index: usize) -> Option<usize> {
        Table::set_cursor(self, index)
    }

    fn cursor_id(&self) -> Option<String> {
        Table::cursor_id(self)
    }

    fn cursor_up(&self) -> Option<(Option<usize>, usize)> {
        Table::cursor_up(self)
    }

    fn cursor_down(&self) -> Option<(Option<usize>, usize)> {
        Table::cursor_down(self)
    }

    fn cursor_first(&self) -> Option<(Option<usize>, usize)> {
        Table::cursor_first(self)
    }

    fn cursor_last(&self) -> Option<(Option<usize>, usize)> {
        Table::cursor_last(self)
    }

    fn scroll_to_cursor(&self) {
        Table::scroll_to_cursor(self)
    }

    fn selection_mode(&self) -> SelectionMode {
        Table::selection_mode(self)
    }

    fn selected_ids(&self) -> Vec<String> {
        Table::selected_ids(self)
    }

    fn toggle_select_at_cursor(&self) -> (Vec<String>, Vec<String>) {
        Table::toggle_select_at_cursor(self)
    }

    fn select_all(&self) -> Vec<String> {
        Table::select_all(self)
    }

    fn deselect_all(&self) -> Vec<String> {
        Table::deselect_all(self)
    }

    fn item_count(&self) -> usize {
        Table::len(self)
    }

    fn viewport_item_count(&self) -> usize {
        Table::viewport_item_count(self)
    }

    fn item_height(&self) -> u16 {
        Table::item_height(self)
    }

    // Override to account for header row
    fn index_from_viewport_y(&self, y: u16) -> Option<usize> {
        // y=0 is header row, data starts at y=1
        if y == 0 {
            return None; // Header row, not a data row
        }
        let data_y = y - 1; // Adjust for header
        let scroll_offset = self.scroll_offset_y();
        let item_height = self.item_height();
        if item_height == 0 {
            return None;
        }
        let absolute_y = scroll_offset + data_y;
        let index = (absolute_y / item_height) as usize;
        if index < self.item_count() {
            Some(index)
        } else {
            None
        }
    }
}
