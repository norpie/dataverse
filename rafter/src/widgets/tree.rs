//! Tree widget - a virtualized hierarchical tree view.
//!
//! This widget only creates Element objects for visible nodes, enabling
//! smooth scrolling with large trees.

use std::collections::HashSet;
use std::hash::Hash;
use std::sync::Arc;

use tuidom::{Color, Element, Overflow, Size, Style, Transitions};

use crate::state::State;
use crate::{HandlerRegistry, WidgetHandlers};

use super::scroll::{
    register_scrollbar_handlers, ScrollRequest, ScrollState, ScrollableWidgetState, Scrollbar,
};
use super::selection::{Selection, SelectionMode};

// =============================================================================
// TreeItem Trait
// =============================================================================

/// Trait for items that can be displayed in a Tree widget.
///
/// # Example
///
/// ```ignore
/// #[derive(Clone)]
/// struct FileNode {
///     path: String,
///     name: String,
///     is_dir: bool,
/// }
///
/// impl TreeItem for FileNode {
///     type Key = String;
///
///     fn key(&self) -> String {
///         self.path.clone()
///     }
///
///     fn render(&self) -> Element {
///         Element::text(&self.name)
///     }
/// }
/// ```
pub trait TreeItem: Clone + Send + Sync + 'static {
    /// The key type used to identify this item. Must be convertible to String
    /// for element ID generation.
    type Key: Clone + Eq + Hash + ToString + Send + Sync + 'static;

    /// Return a unique key for this item.
    fn key(&self) -> Self::Key;

    /// Render this item as an Element.
    /// The Tree widget adds indentation and expand/collapse icons.
    fn render(&self) -> Element;

    /// Height of this item in rows for virtualization.
    fn height(&self) -> u16 {
        1 // Default: 1 row
    }
}

// =============================================================================
// TreeNode
// =============================================================================

/// A node in the tree with a value and children.
#[derive(Clone, Debug)]
pub struct TreeNode<T: TreeItem> {
    /// The item value at this node.
    pub value: T,
    /// Child nodes.
    pub children: Vec<TreeNode<T>>,
}

impl<T: TreeItem> TreeNode<T> {
    /// Create a new leaf node (no children).
    pub fn leaf(value: T) -> Self {
        Self {
            value,
            children: Vec::new(),
        }
    }

    /// Create a new branch node with children.
    pub fn branch(value: T, children: Vec<TreeNode<T>>) -> Self {
        Self { value, children }
    }

    /// Check if this node has children.
    pub fn has_children(&self) -> bool {
        !self.children.is_empty()
    }
}

// =============================================================================
// FlatNode (internal)
// =============================================================================

/// A flattened representation of a tree node for virtualization.
#[derive(Clone, Debug)]
struct FlatNode<K: Clone> {
    /// The node's key.
    key: K,
    /// Depth in the tree (0 = root level).
    depth: usize,
    /// Whether this node has children.
    has_children: bool,
    /// Whether this node is currently expanded.
    is_expanded: bool,
    /// Parent node's key (None for root nodes).
    parent_key: Option<K>,
    /// Index of the first child in the flattened list (if expanded and has children).
    first_child_index: Option<usize>,
}

// =============================================================================
// TreeState
// =============================================================================

/// State for a virtualized Tree widget.
///
/// Uses cumulative height caching for O(1) position lookups and O(log n)
/// offset-to-index queries.
#[derive(Clone, Debug)]
pub struct TreeState<T: TreeItem> {
    /// The root nodes of the tree.
    pub roots: Vec<TreeNode<T>>,
    /// Set of expanded node keys.
    pub expanded: HashSet<T::Key>,
    /// Selection state.
    pub selection: Selection<T::Key>,
    /// Scroll state for virtualization.
    pub scroll: ScrollState,
    /// The key of the last activated item.
    pub last_activated: Option<T::Key>,

    /// Cached flattened view of visible nodes.
    flattened: Vec<FlatNode<T::Key>>,

    /// Cached cumulative heights for O(1) position lookups.
    cumulative_heights: Vec<u16>,

    /// Scrollbar screen rect for drag calculations.
    scrollbar_rect: Option<(u16, u16, u16, u16)>,

    /// Grab offset within thumb for smooth dragging.
    drag_grab_offset: Option<u16>,
}

impl<T: TreeItem> Default for TreeState<T> {
    fn default() -> Self {
        Self {
            roots: Vec::new(),
            expanded: HashSet::new(),
            selection: Selection::none(),
            scroll: ScrollState::new(),
            last_activated: None,
            flattened: Vec::new(),
            cumulative_heights: vec![0],
            scrollbar_rect: None,
            drag_grab_offset: None,
        }
    }
}

impl<T: TreeItem> TreeState<T> {
    /// Create a new TreeState with the given root nodes.
    pub fn new(roots: Vec<TreeNode<T>>) -> Self {
        let mut state = Self::default();
        state.set_roots(roots);
        state
    }

    /// Set the selection mode.
    pub fn with_selection(mut self, mode: SelectionMode) -> Self {
        self.selection = match mode {
            SelectionMode::None => Selection::none(),
            SelectionMode::Single => Selection::single(),
            SelectionMode::Multi => Selection::multi(),
        };
        self
    }

    /// Expand all root nodes initially.
    pub fn with_roots_expanded(mut self) -> Self {
        for root in &self.roots {
            self.expanded.insert(root.value.key());
        }
        self.rebuild_flattened();
        self
    }

    /// Set root nodes and rebuild the flattened view.
    pub fn set_roots(&mut self, roots: Vec<TreeNode<T>>) {
        self.roots = roots;
        self.rebuild_flattened();
    }

    /// Toggle the expanded state of a node.
    pub fn toggle_expanded(&mut self, key: &T::Key) {
        if self.expanded.contains(key) {
            self.expanded.remove(key);
        } else {
            self.expanded.insert(key.clone());
        }
        self.rebuild_flattened();
    }

    /// Expand a node.
    pub fn expand(&mut self, key: &T::Key) {
        if self.expanded.insert(key.clone()) {
            self.rebuild_flattened();
        }
    }

    /// Collapse a node.
    pub fn collapse(&mut self, key: &T::Key) {
        if self.expanded.remove(key) {
            self.rebuild_flattened();
        }
    }

    /// Check if a node is expanded.
    pub fn is_expanded(&self, key: &T::Key) -> bool {
        self.expanded.contains(key)
    }

    /// Rebuild the flattened view and cumulative heights.
    fn rebuild_flattened(&mut self) {
        self.flattened.clear();
        self.cumulative_heights.clear();
        self.cumulative_heights.push(0);

        self.flatten_nodes(&self.roots.clone(), 0, None);

        let total_height = self.cumulative_heights.last().copied().unwrap_or(0);
        self.scroll.set_content_height(total_height);
    }

    /// Recursively flatten nodes into the flattened list.
    fn flatten_nodes(
        &mut self,
        nodes: &[TreeNode<T>],
        depth: usize,
        parent_key: Option<T::Key>,
    ) {
        for node in nodes {
            let key = node.value.key();
            let is_expanded = self.expanded.contains(&key);
            let has_children = node.has_children();

            let height = node.value.height();
            let cumulative = self.cumulative_heights.last().unwrap() + height;
            self.cumulative_heights.push(cumulative);

            let current_index = self.flattened.len();

            self.flattened.push(FlatNode {
                key: key.clone(),
                depth,
                has_children,
                is_expanded,
                parent_key: parent_key.clone(),
                first_child_index: None, // Will be set below if expanded
            });

            if is_expanded && has_children {
                // Record the index of the first child
                let first_child_idx = self.flattened.len();
                self.flattened[current_index].first_child_index = Some(first_child_idx);

                self.flatten_nodes(&node.children, depth + 1, Some(key));
            }
        }
    }

    /// Get the number of visible (flattened) nodes.
    pub fn visible_count(&self) -> usize {
        self.flattened.len()
    }

    /// Get Y offset for node at index. O(1).
    pub fn node_y_offset(&self, index: usize) -> u16 {
        self.cumulative_heights.get(index).copied().unwrap_or(0)
    }

    /// Get total content height. O(1).
    pub fn total_height(&self) -> u16 {
        self.cumulative_heights.last().copied().unwrap_or(0)
    }

    /// Find node index at given Y offset. O(log n) binary search.
    pub fn node_at_offset(&self, y: u16) -> usize {
        self.cumulative_heights
            .partition_point(|&h| h <= y)
            .saturating_sub(1)
    }

    /// Get height of node at index. O(1).
    pub fn node_height(&self, index: usize) -> u16 {
        if index + 1 < self.cumulative_heights.len() {
            self.cumulative_heights[index + 1] - self.cumulative_heights[index]
        } else {
            1
        }
    }

    /// Find the index of a node by key.
    pub fn index_of(&self, key: &T::Key) -> Option<usize> {
        self.flattened.iter().position(|n| &n.key == key)
    }

    /// Find the actual TreeNode by key (searches recursively).
    pub fn find_node(&self, key: &T::Key) -> Option<&TreeNode<T>> {
        fn find_in_nodes<'a, T: TreeItem>(
            nodes: &'a [TreeNode<T>],
            key: &T::Key,
        ) -> Option<&'a TreeNode<T>> {
            for node in nodes {
                if &node.value.key() == key {
                    return Some(node);
                }
                if let Some(found) = find_in_nodes(&node.children, key) {
                    return Some(found);
                }
            }
            None
        }
        find_in_nodes(&self.roots, key)
    }

    /// Process any pending scroll request.
    pub fn process_scroll(&mut self) -> bool {
        let old_offset = self.scroll.offset;

        if let Some(request) = self.scroll.process_request() {
            if let ScrollRequest::IntoView(index) = request {
                let y = self.node_y_offset(index);
                let node_h = self.node_height(index);
                let viewport = self.scroll.viewport;
                let offset = self.scroll.offset;

                if y < offset {
                    self.scroll.offset = y;
                } else if y + node_h > offset + viewport {
                    self.scroll.offset = (y + node_h).saturating_sub(viewport);
                }
            }
        }

        self.scroll.offset != old_offset
    }

    /// Get the index of the first visible node based on current scroll offset.
    pub fn first_visible_index(&self) -> usize {
        self.node_at_offset(self.scroll.offset)
    }
}

// Implement ScrollableWidgetState for TreeState
impl<T: TreeItem> ScrollableWidgetState for TreeState<T> {
    fn scroll(&self) -> &ScrollState {
        &self.scroll
    }

    fn scroll_mut(&mut self) -> &mut ScrollState {
        &mut self.scroll
    }

    fn scrollbar_rect(&self) -> Option<(u16, u16, u16, u16)> {
        self.scrollbar_rect
    }

    fn set_scrollbar_rect(&mut self, rect: Option<(u16, u16, u16, u16)>) {
        self.scrollbar_rect = rect;
    }

    fn drag_grab_offset(&self) -> Option<u16> {
        self.drag_grab_offset
    }

    fn set_drag_grab_offset(&mut self, offset: Option<u16>) {
        self.drag_grab_offset = offset;
    }
}

// =============================================================================
// VisibleNode (internal)
// =============================================================================

/// Information about a visible node for rendering.
struct VisibleNode {
    /// Index in the flattened list.
    index: usize,
}

// =============================================================================
// Tree Widget
// =============================================================================

/// Typestate marker: tree needs a state reference.
pub struct NeedsState;

/// Typestate marker: tree has a state reference.
pub struct HasTreeState<'a, T: TreeItem>(pub(crate) &'a State<TreeState<T>>);

/// A virtualized tree widget builder.
///
/// Uses typestate pattern to enforce `state()` is called before `build()`.
pub struct Tree<S = NeedsState> {
    state_marker: S,
    id: Option<String>,
    style: Option<Style>,
    node_style: Option<Style>,
    node_style_selected: Option<Style>,
    node_style_focused: Option<Style>,
    transitions: Option<Transitions>,
    /// Indentation per depth level (default: 2).
    indent: u16,
    /// Icon for collapsed nodes (default: "▶").
    expand_icon: String,
    /// Icon for expanded nodes (default: "▼").
    collapse_icon: String,
    /// Icon for leaf nodes (default: " ").
    leaf_icon: String,
    /// Whether to show scrollbar (default: true).
    show_scrollbar: bool,
}

impl Default for Tree<NeedsState> {
    fn default() -> Self {
        Self::new()
    }
}

impl Tree<NeedsState> {
    /// Create a new tree builder.
    pub fn new() -> Self {
        Self {
            state_marker: NeedsState,
            id: None,
            style: None,
            node_style: None,
            node_style_selected: None,
            node_style_focused: None,
            transitions: None,
            indent: 2,
            expand_icon: "▶".to_string(),
            collapse_icon: "▼".to_string(),
            leaf_icon: " ".to_string(),
            show_scrollbar: true,
        }
    }

    /// Set the state reference. Required before calling `build()`.
    pub fn state<T: TreeItem>(self, s: &State<TreeState<T>>) -> Tree<HasTreeState<'_, T>> {
        Tree {
            state_marker: HasTreeState(s),
            id: self.id,
            style: self.style,
            node_style: self.node_style,
            node_style_selected: self.node_style_selected,
            node_style_focused: self.node_style_focused,
            transitions: self.transitions,
            indent: self.indent,
            expand_icon: self.expand_icon,
            collapse_icon: self.collapse_icon,
            leaf_icon: self.leaf_icon,
            show_scrollbar: self.show_scrollbar,
        }
    }
}

impl<S> Tree<S> {
    /// Set the tree id.
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Set the tree container style.
    pub fn style(mut self, s: Style) -> Self {
        self.style = Some(s);
        self
    }

    /// Set the style for each node row.
    pub fn node_style(mut self, s: Style) -> Self {
        self.node_style = Some(s);
        self
    }

    /// Set the style for selected nodes.
    pub fn node_style_selected(mut self, s: Style) -> Self {
        self.node_style_selected = Some(s);
        self
    }

    /// Set the style when a node is focused.
    pub fn node_style_focused(mut self, s: Style) -> Self {
        self.node_style_focused = Some(s);
        self
    }

    /// Set transitions.
    pub fn transitions(mut self, t: Transitions) -> Self {
        self.transitions = Some(t);
        self
    }

    /// Set indentation per depth level.
    pub fn indent(mut self, chars: u16) -> Self {
        self.indent = chars;
        self
    }

    /// Set icons for expand/collapse/leaf states.
    pub fn icons(mut self, expand: &str, collapse: &str, leaf: &str) -> Self {
        self.expand_icon = expand.to_string();
        self.collapse_icon = collapse.to_string();
        self.leaf_icon = leaf.to_string();
        self
    }

    /// Set whether to show the scrollbar.
    pub fn show_scrollbar(mut self, show: bool) -> Self {
        self.show_scrollbar = show;
        self
    }
}

impl<'a, T: TreeItem> Tree<HasTreeState<'a, T>> {
    /// Build the tree element.
    pub fn build(self, registry: &HandlerRegistry, handlers: &WidgetHandlers) -> Element {
        let state = self.state_marker.0;
        let tree_id = self.id.clone().unwrap_or_else(|| "tree".into());
        let content_id = format!("{}-content", tree_id);

        // Process any pending scroll request
        state.update(|s| {
            s.process_scroll();
        });

        let current = state.get();

        // Calculate visible range
        let visible = self.calculate_visible_nodes(&current);

        // Create elements only for visible nodes
        let mut children = Vec::with_capacity(visible.len());

        let visible_count = visible.len();
        let total_nodes = current.visible_count();
        for (pos_in_visible, vis_node) in visible.iter().enumerate() {
            let flat_node = &current.flattened[vis_node.index];
            let tree_node = current.find_node(&flat_node.key);

            if let Some(node) = tree_node {
                let row = self.build_node_row(
                    node,
                    flat_node,
                    vis_node.index,
                    pos_in_visible,
                    visible_count,
                    total_nodes,
                    &current.selection,
                    &tree_id,
                    registry,
                    handlers,
                    state,
                );
                children.push(row);
            }
        }

        // Build content column
        let mut content = Element::col()
            .id(&content_id)
            .width(Size::Fill)
            .height(Size::Fill)
            .overflow_x(Overflow::Hidden)
            .overflow_y(Overflow::Hidden)
            .scrollable(true)
            .children(children);

        if let Some(ref style) = self.style {
            content = content.style(style.clone());
        }
        if let Some(ref transitions) = self.transitions {
            content = content.transitions(transitions.clone());
        }

        // Register on_scroll handler
        {
            let state_clone = state.clone();
            let tree_id_clone = tree_id.clone();
            registry.register(
                &content_id,
                "on_scroll",
                Arc::new(move |hx| {
                    // Mouse wheel: delta
                    if let Some((_, delta_y)) = hx.event().scroll_delta() {
                        state_clone.update(|s| {
                            s.scroll.scroll_by(delta_y);
                        });
                    }
                    // Page Up/Down/Home/End
                    if let Some(action) = hx.event().scroll_action() {
                        state_clone.update(|s| {
                            let scroll_request = match action {
                                tuidom::ScrollAction::PageUp => ScrollRequest::PageUp,
                                tuidom::ScrollAction::PageDown => ScrollRequest::PageDown,
                                tuidom::ScrollAction::Home => ScrollRequest::Home,
                                tuidom::ScrollAction::End => ScrollRequest::End,
                            };
                            s.scroll.apply_request(scroll_request);
                        });

                        let current = state_clone.get();
                        if current.flattened.is_empty() {
                            return;
                        }

                        let target_index = match action {
                            tuidom::ScrollAction::Home => 0,
                            tuidom::ScrollAction::End => current.flattened.len() - 1,
                            tuidom::ScrollAction::PageUp => current.first_visible_index(),
                            tuidom::ScrollAction::PageDown => {
                                let first = current.first_visible_index();
                                let viewport = current.scroll.viewport as usize;
                                (first + viewport.saturating_sub(1)).min(current.flattened.len() - 1)
                            }
                        };

                        if let Some(flat_node) = current.flattened.get(target_index) {
                            let node_id =
                                format!("{}-node-{}", tree_id_clone, flat_node.key.to_string());
                            hx.cx().focus(&node_id);
                        }
                    }
                }),
            );
        }

        // Register on_layout handler for viewport discovery
        {
            let state_clone = state.clone();
            registry.register(
                &content_id,
                "on_layout",
                Arc::new(move |hx| {
                    if let Some((_, _, _, height)) = hx.event().layout() {
                        state_clone.update(|s| {
                            s.scroll.set_viewport(height);
                        });
                    }
                }),
            );
        }

        // Add scrollbar if needed
        let show_scrollbar = self.show_scrollbar && current.scroll.can_scroll();
        if show_scrollbar {
            let scrollbar_id = format!("{}-scrollbar", tree_id);
            let scrollbar = Scrollbar::vertical()
                .id(&scrollbar_id)
                .scroll_state(&current.scroll)
                .build();

            register_scrollbar_handlers(&scrollbar_id, registry, state);

            Element::row()
                .id(&tree_id)
                .width(Size::Fill)
                .height(Size::Fill)
                .child(content)
                .child(scrollbar)
        } else {
            content.id(&tree_id)
        }
    }

    /// Calculate which nodes are visible given current scroll state.
    fn calculate_visible_nodes(&self, state: &TreeState<T>) -> Vec<VisibleNode> {
        let scroll_y = state.scroll.offset;
        let viewport = state.scroll.viewport;

        if state.flattened.is_empty() {
            return Vec::new();
        }

        let effective_viewport = if viewport == 0 { 200 } else { viewport };

        let first_visible = state.node_at_offset(scroll_y);

        let mut end_idx = first_visible;
        let mut total_height: u16 = 0;
        while end_idx < state.flattened.len() && total_height < effective_viewport {
            total_height += state.node_height(end_idx);
            end_idx += 1;
        }

        let mut nodes = Vec::with_capacity(end_idx - first_visible);
        for i in first_visible..end_idx {
            nodes.push(VisibleNode { index: i });
        }

        nodes
    }

    /// Build a single node row element.
    #[allow(clippy::too_many_arguments)]
    fn build_node_row(
        &self,
        node: &TreeNode<T>,
        flat_node: &FlatNode<T::Key>,
        node_index: usize,
        pos_in_visible: usize,
        visible_count: usize,
        total_nodes: usize,
        selection: &Selection<T::Key>,
        tree_id: &str,
        registry: &HandlerRegistry,
        handlers: &WidgetHandlers,
        state: &State<TreeState<T>>,
    ) -> Element {
        let key = flat_node.key.clone();
        let row_id = format!("{}-node-{}", tree_id, key.to_string());
        let is_selected = selection.is_selected(&key);

        // Check boundary conditions
        let is_at_top_boundary = pos_in_visible == 0 && node_index > 0;
        let is_at_bottom_boundary =
            pos_in_visible == visible_count - 1 && node_index < total_nodes - 1;

        // Build indentation
        let indent_width = (flat_node.depth as u16) * self.indent;
        let indent_str = " ".repeat(indent_width as usize);

        // Choose icon
        let icon = if flat_node.has_children {
            if flat_node.is_expanded {
                &self.collapse_icon
            } else {
                &self.expand_icon
            }
        } else {
            &self.leaf_icon
        };

        // Build the row: [indent][icon][content]
        let icon_id = format!("{}-icon", row_id);

        let mut row = Element::row()
            .id(&row_id)
            .width(Size::Fill)
            .focusable(true)
            .clickable(true)
            .child(Element::text(&indent_str))
            .child(
                Element::text(icon)
                    .id(&icon_id)
                    .clickable(flat_node.has_children),
            )
            .child(Element::text(" ")) // Spacing between icon and content
            .child(node.value.render());

        // Apply base node style
        if let Some(ref style) = self.node_style {
            row = row.style(style.clone());
        }

        // Apply selected style
        if is_selected {
            if let Some(ref style) = self.node_style_selected {
                row = row.style(style.clone());
            } else {
                row = row.style(
                    Style::new()
                        .background(Color::var("tree.node_selected"))
                        .foreground(Color::var("text.inverted")),
                );
            }
        }

        // Apply focused style
        if let Some(ref style) = self.node_style_focused {
            row = row.style_focused(style.clone());
        } else {
            row = row.style_focused(
                Style::new()
                    .background(Color::var("tree.node_focused"))
                    .foreground(Color::var("text.inverted")),
            );
        }

        // Set explicit height
        row = row.height(Size::Fixed(node.value.height()));

        // Register icon click handler for expand/collapse
        if flat_node.has_children {
            let state_clone = state.clone();
            let key_clone = key.clone();
            registry.register(
                &icon_id,
                "on_activate",
                Arc::new(move |_hx| {
                    state_clone.update(|s| {
                        s.toggle_expanded(&key_clone);
                    });
                }),
            );
        }

        // Register boundary scroll handlers
        if is_at_top_boundary {
            let state_clone = state.clone();
            let tree_id_clone = tree_id.to_string();
            let target_index = node_index.saturating_sub(1);
            registry.register(
                &row_id,
                "on_key_up",
                Arc::new(move |hx| {
                    state_clone.update(|s| {
                        s.scroll.apply_request(ScrollRequest::Delta(-1));
                    });
                    let current = state_clone.get();
                    if let Some(flat_node) = current.flattened.get(target_index) {
                        let node_id =
                            format!("{}-node-{}", tree_id_clone, flat_node.key.to_string());
                        hx.cx().focus(&node_id);
                    }
                }),
            );
        }

        if is_at_bottom_boundary {
            let state_clone = state.clone();
            let tree_id_clone = tree_id.to_string();
            let target_index = node_index + 1;
            registry.register(
                &row_id,
                "on_key_down",
                Arc::new(move |hx| {
                    state_clone.update(|s| {
                        s.scroll.apply_request(ScrollRequest::Delta(1));
                    });
                    let current = state_clone.get();
                    if let Some(flat_node) = current.flattened.get(target_index) {
                        let node_id =
                            format!("{}-node-{}", tree_id_clone, flat_node.key.to_string());
                        hx.cx().focus(&node_id);
                    }
                }),
            );
        }

        // Register Left arrow handler: collapse or go to parent
        {
            let state_clone = state.clone();
            let tree_id_clone = tree_id.to_string();
            let key_clone = key.clone();
            let is_expanded = flat_node.is_expanded;
            let has_children = flat_node.has_children;
            let parent_key = flat_node.parent_key.clone();

            registry.register(
                &row_id,
                "on_key_left",
                Arc::new(move |hx| {
                    if is_expanded && has_children {
                        // Collapse this node
                        state_clone.update(|s| {
                            s.collapse(&key_clone);
                        });
                    } else if let Some(ref parent) = parent_key {
                        // Go to parent
                        let node_id = format!("{}-node-{}", tree_id_clone, parent.to_string());
                        hx.cx().focus(&node_id);
                    }
                    // If at root and collapsed, do nothing (don't escape tree)
                }),
            );
        }

        // Register Right arrow handler: expand or go to first child
        {
            let state_clone = state.clone();
            let tree_id_clone = tree_id.to_string();
            let key_clone = key.clone();
            let is_expanded = flat_node.is_expanded;
            let has_children = flat_node.has_children;
            let first_child_index = flat_node.first_child_index;

            registry.register(
                &row_id,
                "on_key_right",
                Arc::new(move |hx| {
                    if has_children {
                        if !is_expanded {
                            // Expand this node
                            state_clone.update(|s| {
                                s.expand(&key_clone);
                            });
                        } else if let Some(child_idx) = first_child_index {
                            // Go to first child
                            let current = state_clone.get();
                            if let Some(child_node) = current.flattened.get(child_idx) {
                                let node_id =
                                    format!("{}-node-{}", tree_id_clone, child_node.key.to_string());
                                hx.cx().focus(&node_id);
                            }
                        }
                    }
                    // If leaf, do nothing (don't escape tree)
                }),
            );
        }

        // Register activation handler (Enter/Space)
        {
            let state_clone = state.clone();
            let key_clone = key.clone();
            let on_select = handlers.get("on_select").cloned();
            let on_activate = handlers.get("on_activate").cloned();

            registry.register(
                &row_id,
                "on_activate",
                Arc::new(move |hx| {
                    state_clone.update(|s| {
                        s.last_activated = Some(key_clone.clone());
                        s.selection.toggle(key_clone.clone());
                    });
                    if let Some(ref handler) = on_select {
                        handler(hx);
                    }
                    if let Some(ref handler) = on_activate {
                        handler(hx);
                    }
                }),
            );
        }

        row
    }
}
