//! Tree widget state.

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};

use crate::widgets::scrollbar::{
    ScrollbarConfig, ScrollbarDrag, ScrollbarGeometry, ScrollbarState,
};
use crate::widgets::selection::{Selection, SelectionMode};
use crate::widgets::traits::{ScrollableWidget, SelectableWidget};

use super::item::TreeItem;

/// Unique identifier for a Tree widget instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TreeId(usize);

impl TreeId {
    fn new() -> Self {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        Self(COUNTER.fetch_add(1, Ordering::SeqCst))
    }
}

impl std::fmt::Display for TreeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "__tree_{}", self.0)
    }
}

/// A visible node in the flattened tree page.
#[derive(Debug, Clone)]
pub struct FlatNode<T: TreeItem> {
    /// The item itself.
    pub item: T,
    /// Depth in tree (0 = root).
    pub depth: u16,
    /// Whether this node has children.
    pub has_children: bool,
    /// Whether this node is currently expanded.
    pub is_expanded: bool,
}

/// Internal state for the Tree widget.
#[derive(Debug)]
pub(super) struct TreeInner<T: TreeItem> {
    /// Root items.
    pub roots: Vec<T>,
    /// Set of expanded node IDs.
    pub expanded: HashSet<String>,
    /// Flattened visible nodes (rebuilt on expand/collapse).
    pub visible: Vec<FlatNode<T>>,
    /// Selection state (by ID).
    pub selection: Selection,
    /// Selection mode.
    pub selection_mode: SelectionMode,
    /// Cursor (index into visible list).
    pub cursor: Option<usize>,
    /// Scroll offset in rows.
    pub scroll_offset: u16,
    /// Viewport height.
    pub viewport_height: u16,
    /// Scrollbar configuration.
    pub scrollbar: ScrollbarConfig,
    /// Vertical scrollbar geometry.
    pub vertical_scrollbar: Option<ScrollbarGeometry>,
    /// Drag state.
    pub drag: Option<ScrollbarDrag>,
}

impl<T: TreeItem> Default for TreeInner<T> {
    fn default() -> Self {
        Self {
            roots: Vec::new(),
            expanded: HashSet::new(),
            visible: Vec::new(),
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

/// A virtualized tree widget with selection and expand/collapse support.
///
/// `Tree<T>` manages hierarchical data with:
/// - Virtualized rendering (only visible nodes are rendered)
/// - Expand/collapse state per node
/// - Cursor navigation
/// - Selection (single or multi-select by node ID)
///
/// # Example
///
/// ```ignore
/// #[derive(Clone)]
/// struct FileNode {
///     path: String,
///     name: String,
///     children: Vec<FileNode>,
/// }
///
/// impl TreeItem for FileNode {
///     fn id(&self) -> String { self.path.clone() }
///     fn children(&self) -> Vec<Self> { self.children.clone() }
///     fn render(&self, focused: bool, selected: bool, depth: u16, expanded: bool) -> Node {
///         // ...
///     }
/// }
///
/// let tree = Tree::with_items(vec![root_node]);
/// tree.expand("/home");
/// ```
#[derive(Debug)]
pub struct Tree<T: TreeItem> {
    /// Unique identifier.
    id: TreeId,
    /// Internal state.
    pub(super) inner: Arc<RwLock<TreeInner<T>>>,
    /// Dirty flag for re-render.
    pub(super) dirty: Arc<AtomicBool>,
}

impl<T: TreeItem> Tree<T> {
    /// Create a new empty tree.
    pub fn new() -> Self {
        Self {
            id: TreeId::new(),
            inner: Arc::new(RwLock::new(TreeInner::default())),
            dirty: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Create a tree with initial root items.
    pub fn with_items(roots: Vec<T>) -> Self {
        let tree = Self {
            id: TreeId::new(),
            inner: Arc::new(RwLock::new(TreeInner {
                roots,
                ..Default::default()
            })),
            dirty: Arc::new(AtomicBool::new(false)),
        };
        // Build initial visible list
        if let Ok(mut guard) = tree.inner.write() {
            Self::rebuild_visible(&mut guard);
        }
        tree
    }

    /// Create a tree with selection mode.
    pub fn with_selection_mode(mode: SelectionMode) -> Self {
        Self {
            id: TreeId::new(),
            inner: Arc::new(RwLock::new(TreeInner {
                selection_mode: mode,
                ..Default::default()
            })),
            dirty: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Get the unique ID.
    pub fn id(&self) -> TreeId {
        self.id
    }

    /// Get the ID as a string.
    pub fn id_string(&self) -> String {
        self.id.to_string()
    }

    // -------------------------------------------------------------------------
    // Item access
    // -------------------------------------------------------------------------

    /// Get the root items.
    pub fn roots(&self) -> Vec<T> {
        self.inner
            .read()
            .map(|g| g.roots.clone())
            .unwrap_or_default()
    }

    /// Set the root items (rebuilds visible list).
    pub fn set_items(&self, roots: Vec<T>) {
        if let Ok(mut guard) = self.inner.write() {
            guard.roots = roots;
            // Keep expanded state, rebuild visible
            Self::rebuild_visible(&mut guard);
            // Clamp cursor
            if let Some(cursor) = guard.cursor
                && cursor >= guard.visible.len()
            {
                guard.cursor = guard.visible.len().checked_sub(1);
            }
            self.dirty.store(true, Ordering::SeqCst);
        }
    }

    /// Get the number of visible nodes.
    pub fn visible_len(&self) -> usize {
        self.inner.read().map(|g| g.visible.len()).unwrap_or(0)
    }

    /// Check if the tree has no items.
    pub fn is_empty(&self) -> bool {
        self.inner
            .read()
            .map(|g| g.roots.is_empty())
            .unwrap_or(true)
    }

    /// Get a visible node by index.
    pub fn visible_node(&self, index: usize) -> Option<FlatNode<T>> {
        self.inner
            .read()
            .ok()
            .and_then(|g| g.visible.get(index).cloned())
    }

    /// Get all visible node IDs in order.
    fn all_visible_ids(guard: &TreeInner<T>) -> Vec<String> {
        guard.visible.iter().map(|n| n.item.id()).collect()
    }

    /// Find a node by ID in the entire tree (including collapsed).
    pub fn find(&self, id: &str) -> Option<T> {
        self.inner
            .read()
            .ok()
            .and_then(|g| Self::find_in_items(&g.roots, id))
    }

    /// Recursively search for a node by ID.
    fn find_in_items(items: &[T], id: &str) -> Option<T> {
        for item in items {
            if item.id() == id {
                return Some(item.clone());
            }
            if let Some(found) = Self::find_in_items(&item.children(), id) {
                return Some(found);
            }
        }
        None
    }

    // -------------------------------------------------------------------------
    // Expand/Collapse
    // -------------------------------------------------------------------------

    /// Expand a node by ID.
    pub fn expand(&self, node_id: &str) {
        if let Ok(mut guard) = self.inner.write()
            && guard.expanded.insert(node_id.to_string())
        {
            Self::rebuild_visible(&mut guard);
            self.dirty.store(true, Ordering::SeqCst);
        }
    }

    /// Collapse a node by ID.
    pub fn collapse(&self, node_id: &str) {
        if let Ok(mut guard) = self.inner.write()
            && guard.expanded.remove(node_id)
        {
            Self::rebuild_visible(&mut guard);
            self.dirty.store(true, Ordering::SeqCst);
        }
    }

    /// Toggle expand/collapse for a node.
    pub fn toggle(&self, node_id: &str) {
        if let Ok(mut guard) = self.inner.write() {
            if guard.expanded.contains(node_id) {
                guard.expanded.remove(node_id);
            } else {
                guard.expanded.insert(node_id.to_string());
            }
            Self::rebuild_visible(&mut guard);
            self.dirty.store(true, Ordering::SeqCst);
        }
    }

    /// Check if a node is expanded.
    pub fn is_expanded(&self, node_id: &str) -> bool {
        self.inner
            .read()
            .map(|g| g.expanded.contains(node_id))
            .unwrap_or(false)
    }

    /// Expand all expandable nodes.
    pub fn expand_all(&self) {
        if let Ok(mut guard) = self.inner.write() {
            let roots = guard.roots.clone();
            Self::collect_all_expandable_ids(&roots, &mut guard.expanded);
            Self::rebuild_visible(&mut guard);
            self.dirty.store(true, Ordering::SeqCst);
        }
    }

    /// Collapse all nodes.
    pub fn collapse_all(&self) {
        if let Ok(mut guard) = self.inner.write() {
            guard.expanded.clear();
            Self::rebuild_visible(&mut guard);
            self.dirty.store(true, Ordering::SeqCst);
        }
    }

    /// Recursively collect IDs of all expandable nodes.
    fn collect_all_expandable_ids(items: &[T], expanded: &mut HashSet<String>) {
        for item in items {
            let children = item.children();
            if !children.is_empty() {
                expanded.insert(item.id());
                Self::collect_all_expandable_ids(&children, expanded);
            }
        }
    }

    /// Rebuild the flattened visible node list.
    fn rebuild_visible(inner: &mut TreeInner<T>) {
        inner.visible.clear();
        Self::collect_visible(&inner.roots, &inner.expanded, 0, &mut inner.visible);

        // Clamp cursor if out of bounds
        if let Some(cursor) = inner.cursor
            && cursor >= inner.visible.len()
        {
            inner.cursor = inner.visible.len().checked_sub(1);
        }

        // Clamp scroll offset
        let max_scroll = Self::max_scroll_offset_inner(inner);
        if inner.scroll_offset > max_scroll {
            inner.scroll_offset = max_scroll;
        }
    }

    /// Recursively collect visible nodes into the flat list.
    fn collect_visible(
        items: &[T],
        expanded: &HashSet<String>,
        depth: u16,
        out: &mut Vec<FlatNode<T>>,
    ) {
        for item in items {
            let id = item.id();
            let children = item.children();
            let has_children = !children.is_empty();
            let is_expanded = expanded.contains(&id);

            out.push(FlatNode {
                item: item.clone(),
                depth,
                has_children,
                is_expanded,
            });

            if is_expanded && has_children {
                Self::collect_visible(&children, expanded, depth + 1, out);
            }
        }
    }

    // -------------------------------------------------------------------------
    // Cursor
    // -------------------------------------------------------------------------

    /// Get the current cursor position (index into visible list).
    pub fn cursor(&self) -> Option<usize> {
        self.inner.read().ok().and_then(|g| g.cursor)
    }

    /// Get the node at the cursor.
    pub fn cursor_node(&self) -> Option<T> {
        self.inner.read().ok().and_then(|g| {
            g.cursor
                .and_then(|i| g.visible.get(i).map(|n| n.item.clone()))
        })
    }

    /// Get the ID of the node at the cursor.
    pub fn cursor_id(&self) -> Option<String> {
        self.inner
            .read()
            .ok()
            .and_then(|g| g.cursor.and_then(|i| g.visible.get(i).map(|n| n.item.id())))
    }

    /// Set the cursor position.
    pub fn set_cursor(&self, index: usize) -> Option<usize> {
        if let Ok(mut guard) = self.inner.write() {
            let previous = guard.cursor;
            if index < guard.visible.len() && previous != Some(index) {
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
            } else if !guard.visible.is_empty() {
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
            let max_index = guard.visible.len().saturating_sub(1);
            if let Some(cursor) = guard.cursor {
                if cursor < max_index {
                    guard.cursor = Some(cursor + 1);
                    self.dirty.store(true, Ordering::SeqCst);
                    return Some((previous, cursor + 1));
                }
            } else if !guard.visible.is_empty() {
                guard.cursor = Some(0);
                self.dirty.store(true, Ordering::SeqCst);
                return Some((None, 0));
            }
        }
        None
    }

    /// Move cursor to first visible node.
    pub fn cursor_first(&self) -> Option<(Option<usize>, usize)> {
        if let Ok(mut guard) = self.inner.write()
            && !guard.visible.is_empty()
        {
            let previous = guard.cursor;
            guard.cursor = Some(0);
            self.dirty.store(true, Ordering::SeqCst);
            return Some((previous, 0));
        }
        None
    }

    /// Move cursor to last visible node.
    pub fn cursor_last(&self) -> Option<(Option<usize>, usize)> {
        if let Ok(mut guard) = self.inner.write()
            && !guard.visible.is_empty()
        {
            let previous = guard.cursor;
            let last = guard.visible.len() - 1;
            guard.cursor = Some(last);
            self.dirty.store(true, Ordering::SeqCst);
            return Some((previous, last));
        }
        None
    }

    /// Move cursor to parent node.
    ///
    /// Searches backwards in the visible list for a node at depth - 1.
    /// Returns None if cursor is at root level or not set.
    pub fn cursor_to_parent(&self) -> Option<(Option<usize>, usize)> {
        let mut guard = self.inner.write().ok()?;
        let cursor = guard.cursor?;
        let current = guard.visible.get(cursor)?;
        if current.depth == 0 {
            return None; // Already at root
        }
        let target_depth = current.depth - 1;

        // Search backwards for parent
        for i in (0..cursor).rev() {
            if let Some(node) = guard.visible.get(i)
                && node.depth == target_depth
            {
                let previous = guard.cursor;
                guard.cursor = Some(i);
                self.dirty.store(true, Ordering::SeqCst);
                return Some((previous, i));
            }
        }
        None
    }

    /// Move cursor to first child of current node.
    ///
    /// Only works if current node is expanded and has children.
    pub fn cursor_to_first_child(&self) -> Option<(Option<usize>, usize)> {
        let mut guard = self.inner.write().ok()?;
        let cursor = guard.cursor?;
        let current = guard.visible.get(cursor)?;
        if !current.is_expanded || !current.has_children {
            return None;
        }

        // First child is the next node (if it's at depth + 1)
        let next_index = cursor + 1;
        let target_depth = current.depth + 1;
        if let Some(next) = guard.visible.get(next_index)
            && next.depth == target_depth
        {
            let previous = guard.cursor;
            guard.cursor = Some(next_index);
            self.dirty.store(true, Ordering::SeqCst);
            return Some((previous, next_index));
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

    /// Get all selected node IDs.
    pub fn selected_ids(&self) -> Vec<String> {
        self.inner
            .read()
            .map(|g| g.selection.selected())
            .unwrap_or_default()
    }

    /// Get all selected nodes.
    pub fn selected_nodes(&self) -> Vec<T> {
        self.inner
            .read()
            .map(|g| {
                let selected = g.selection.selected();
                g.visible
                    .iter()
                    .filter(|n| selected.contains(&n.item.id()))
                    .map(|n| n.item.clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Check if a node is selected by ID.
    pub fn is_selected(&self, id: &str) -> bool {
        self.inner
            .read()
            .map(|g| g.selection.is_selected(id))
            .unwrap_or(false)
    }

    /// Check if the node at a visible index is selected.
    pub fn is_selected_at(&self, visible_index: usize) -> bool {
        self.inner
            .read()
            .map(|g| {
                g.visible
                    .get(visible_index)
                    .map(|n| g.selection.is_selected(&n.item.id()))
                    .unwrap_or(false)
            })
            .unwrap_or(false)
    }

    /// Select a node by ID (clears other selection in Single mode).
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

    /// Toggle selection of a node by ID.
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

    /// Toggle selection of the node at the cursor.
    /// Returns (added IDs, removed IDs).
    pub fn toggle_select_at_cursor(&self) -> (Vec<String>, Vec<String>) {
        if let Some(id) = self.cursor_id() {
            self.toggle_select(&id)
        } else {
            (vec![], vec![])
        }
    }

    /// Range select from anchor to target ID.
    pub fn range_select(&self, id: &str, extend: bool) -> (Vec<String>, Vec<String>) {
        if let Ok(mut guard) = self.inner.write()
            && guard.selection_mode == SelectionMode::Multiple
        {
            let all_ids = Self::all_visible_ids(&guard);
            let result = guard.selection.range_select(id, &all_ids, extend);
            self.dirty.store(true, Ordering::SeqCst);
            return result;
        }
        (vec![], vec![])
    }

    /// Select all visible nodes.
    pub fn select_all(&self) -> Vec<String> {
        if let Ok(mut guard) = self.inner.write()
            && guard.selection_mode == SelectionMode::Multiple
            && !guard.visible.is_empty()
        {
            let all_ids = Self::all_visible_ids(&guard);
            let result = guard.selection.select_all(&all_ids);
            self.dirty.store(true, Ordering::SeqCst);
            return result;
        }
        vec![]
    }

    /// Clear all selection.
    pub fn deselect_all(&self) -> Vec<String> {
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

    /// Scroll to make a node visible by its visible index.
    pub fn scroll_to_index(&self, index: usize) {
        if let Ok(mut guard) = self.inner.write() {
            if index >= guard.visible.len() {
                return;
            }
            let item_height = T::HEIGHT;
            let item_top = index as u16 * item_height;
            let item_bottom = item_top + item_height;
            let viewport = guard.viewport_height;

            if viewport == 0 {
                return;
            }

            if item_top < guard.scroll_offset {
                guard.scroll_offset = item_top;
                self.dirty.store(true, Ordering::SeqCst);
            } else if item_bottom > guard.scroll_offset + viewport {
                guard.scroll_offset = item_bottom.saturating_sub(viewport);
                self.dirty.store(true, Ordering::SeqCst);
            }
        }
    }

    /// Scroll to make the cursor visible.
    pub fn scroll_to_cursor(&self) {
        if let Some(cursor) = self.cursor() {
            self.scroll_to_index(cursor);
        }
    }

    /// Scroll to make a node visible by ID.
    pub fn scroll_to_node(&self, id: &str) {
        if let Ok(guard) = self.inner.read()
            && let Some(index) = guard.visible.iter().position(|n| n.item.id() == id)
        {
            drop(guard);
            self.scroll_to_index(index);
        }
    }

    fn max_scroll_offset_inner(inner: &TreeInner<T>) -> u16 {
        let total_height = inner.visible.len() as u16 * T::HEIGHT;
        total_height.saturating_sub(inner.viewport_height)
    }

    // -------------------------------------------------------------------------
    // Viewport
    // -------------------------------------------------------------------------

    /// Set the viewport height (called by renderer).
    pub fn set_viewport_height(&self, height: u16) {
        if let Ok(mut guard) = self.inner.write() {
            guard.viewport_height = height;
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

    /// Get the visible node range for rendering.
    pub fn visible_range(&self) -> std::ops::Range<usize> {
        self.inner
            .read()
            .map(|g| Self::visible_range_inner(&g))
            .unwrap_or(0..0)
    }

    fn visible_range_inner(g: &TreeInner<T>) -> std::ops::Range<usize> {
        if g.visible.is_empty() || g.viewport_height == 0 {
            return 0..0;
        }
        let item_height = T::HEIGHT;
        let start = (g.scroll_offset / item_height) as usize;
        let visible_count = g.viewport_height.div_ceil(item_height) as usize;
        let end = (start + visible_count + 1).min(g.visible.len());
        start..end
    }

    /// Get total content height.
    pub fn total_height(&self) -> u16 {
        self.visible_len() as u16 * T::HEIGHT
    }

    /// Get the item height (from the item type).
    pub fn item_height(&self) -> u16 {
        T::HEIGHT
    }

    /// Get the number of items that fit in the viewport.
    pub fn viewport_item_count(&self) -> usize {
        (self.viewport_height() / T::HEIGHT) as usize
    }

    // -------------------------------------------------------------------------
    // Dirty tracking
    // -------------------------------------------------------------------------

    /// Check if the tree has changed.
    pub fn is_dirty(&self) -> bool {
        self.dirty.load(Ordering::SeqCst)
    }

    /// Clear the dirty flag.
    pub fn clear_dirty(&self) {
        self.dirty.store(false, Ordering::SeqCst);
    }
}

impl<T: TreeItem> Clone for Tree<T> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            inner: Arc::clone(&self.inner),
            dirty: Arc::clone(&self.dirty),
        }
    }
}

impl<T: TreeItem> Default for Tree<T> {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// ScrollbarState trait implementation
// =============================================================================

impl<T: TreeItem> ScrollbarState for Tree<T> {
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
        Tree::viewport_height(self)
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

impl<T: TreeItem> ScrollableWidget for Tree<T> {
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

impl<T: TreeItem> SelectableWidget for Tree<T> {
    fn cursor(&self) -> Option<usize> {
        Tree::cursor(self)
    }

    fn set_cursor(&self, index: usize) -> Option<usize> {
        Tree::set_cursor(self, index)
    }

    fn cursor_id(&self) -> Option<String> {
        Tree::cursor_id(self)
    }

    fn cursor_up(&self) -> Option<(Option<usize>, usize)> {
        Tree::cursor_up(self)
    }

    fn cursor_down(&self) -> Option<(Option<usize>, usize)> {
        Tree::cursor_down(self)
    }

    fn cursor_first(&self) -> Option<(Option<usize>, usize)> {
        Tree::cursor_first(self)
    }

    fn cursor_last(&self) -> Option<(Option<usize>, usize)> {
        Tree::cursor_last(self)
    }

    fn scroll_to_cursor(&self) {
        Tree::scroll_to_cursor(self)
    }

    fn selection_mode(&self) -> SelectionMode {
        Tree::selection_mode(self)
    }

    fn selected_ids(&self) -> Vec<String> {
        Tree::selected_ids(self)
    }

    fn toggle_select_at_cursor(&self) -> (Vec<String>, Vec<String>) {
        Tree::toggle_select_at_cursor(self)
    }

    fn select_all(&self) -> Vec<String> {
        Tree::select_all(self)
    }

    fn deselect_all(&self) -> Vec<String> {
        Tree::deselect_all(self)
    }

    fn item_count(&self) -> usize {
        Tree::visible_len(self)
    }

    fn viewport_item_count(&self) -> usize {
        Tree::viewport_item_count(self)
    }

    fn item_height(&self) -> u16 {
        Tree::item_height(self)
    }
}
