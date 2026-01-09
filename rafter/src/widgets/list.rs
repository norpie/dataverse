//! List widget - a virtualized selectable list of items.
//!
//! This widget only creates Element objects for visible items, enabling
//! smooth scrolling with 100K+ items.

use std::hash::Hash;
use std::sync::Arc;

use tuidom::{Color, Element, Size, Style, Transitions};

use crate::state::State;
use crate::{HandlerRegistry, WidgetHandlers};

use super::scroll::{ScrollRequest, ScrollState, Scrollbar};
use super::selection::{Selection, SelectionMode};

// =============================================================================
// ListItem Trait
// =============================================================================

/// Trait for items that can be displayed in a List widget.
///
/// # Example
///
/// ```ignore
/// #[derive(Clone)]
/// struct FileItem {
///     path: String,
///     name: String,
/// }
///
/// impl ListItem for FileItem {
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
pub trait ListItem: Clone + Send + Sync + 'static {
    /// The key type used to identify this item. Must be convertible to String
    /// for element ID generation.
    type Key: Clone + Eq + Hash + ToString + Send + Sync + 'static;

    /// Return a unique key for this item.
    fn key(&self) -> Self::Key;

    /// Render this item as an Element.
    fn render(&self) -> Element;

    /// Height of this item in rows for virtualization.
    ///
    /// Used to calculate scroll positions and visible ranges.
    /// Must match the actual rendered height for correct scrolling behavior.
    fn height(&self) -> u16 {
        1 // Default: 1 row
    }
}

// =============================================================================
// ListState
// =============================================================================

/// State for a virtualized List widget.
///
/// Uses cumulative height caching for O(1) position lookups and O(log n)
/// offset-to-index queries.
///
/// # Example
///
/// ```ignore
/// // In app struct (wrapped in State<> by #[app] macro):
/// files: ListState<FileItem>,
///
/// // Initialize in on_start:
/// self.files.set(ListState::new(vec![
///     FileItem { path: "/a".into(), name: "File A".into() },
///     FileItem { path: "/b".into(), name: "File B".into() },
/// ]).with_selection(SelectionMode::Single));
/// ```
#[derive(Clone, Debug)]
pub struct ListState<T: ListItem> {
    /// The items in the list.
    pub items: Vec<T>,
    /// Selection state.
    pub selection: Selection<T::Key>,
    /// Scroll state for virtualization.
    pub scroll: ScrollState,
    /// The key of the last activated item. Set before handlers are called.
    pub last_activated: Option<T::Key>,

    /// Cached cumulative heights for O(1) position lookups.
    /// `cumulative[i]` = total height of items `0..i`
    /// `cumulative[0]` = 0, `cumulative[n]` = total content height
    /// Length = `items.len() + 1`
    cumulative_heights: Vec<u16>,
}

impl<T: ListItem> Default for ListState<T> {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            selection: Selection::none(),
            scroll: ScrollState::new(),
            last_activated: None,
            cumulative_heights: vec![0],
        }
    }
}

impl<T: ListItem> ListState<T> {
    /// Create a new ListState with the given items.
    pub fn new(items: Vec<T>) -> Self {
        let mut state = Self {
            items: Vec::new(),
            selection: Selection::none(),
            scroll: ScrollState::new(),
            last_activated: None,
            cumulative_heights: vec![0],
        };
        state.set_items(items);
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

    /// Set items and rebuild cumulative height cache.
    ///
    /// O(n) but only called when items change, not every frame.
    pub fn set_items(&mut self, items: Vec<T>) {
        self.cumulative_heights = Vec::with_capacity(items.len() + 1);
        self.cumulative_heights.push(0);

        let mut total: u16 = 0;
        for item in &items {
            total = total.saturating_add(item.height());
            self.cumulative_heights.push(total);
        }

        self.items = items;
        self.scroll.set_content_height(total);
    }

    /// Get Y offset for item at index. O(1).
    pub fn item_y_offset(&self, index: usize) -> u16 {
        self.cumulative_heights.get(index).copied().unwrap_or(0)
    }

    /// Get total content height. O(1).
    pub fn total_height(&self) -> u16 {
        self.cumulative_heights.last().copied().unwrap_or(0)
    }

    /// Find item index at given Y offset. O(log n) binary search.
    pub fn item_at_offset(&self, y: u16) -> usize {
        self.cumulative_heights
            .partition_point(|&h| h <= y)
            .saturating_sub(1)
    }

    /// Get height of item at index. O(1).
    pub fn item_height(&self, index: usize) -> u16 {
        if index + 1 < self.cumulative_heights.len() {
            self.cumulative_heights[index + 1] - self.cumulative_heights[index]
        } else {
            1 // Default
        }
    }

    /// Get the index of an item by key.
    pub fn index_of(&self, key: &T::Key) -> Option<usize> {
        self.items.iter().position(|item| &item.key() == key)
    }

    /// Scroll to make item at index visible.
    pub fn scroll_to_item(&mut self, index: usize) {
        self.scroll.scroll_into_view(index);
    }

    /// Process any pending scroll request.
    ///
    /// Call this at the start of build to apply scroll requests.
    /// Returns true if scroll position changed.
    pub fn process_scroll(&mut self) -> bool {
        let old_offset = self.scroll.offset;

        if let Some(request) = self.scroll.process_request() {
            // Handle IntoView specially since it needs item positions
            if let ScrollRequest::IntoView(index) = request {
                let y = self.item_y_offset(index);
                let item_h = self.item_height(index);
                let viewport = self.scroll.viewport;
                let offset = self.scroll.offset;

                if y < offset {
                    // Item above viewport - scroll up to show it
                    self.scroll.offset = y;
                } else if y + item_h > offset + viewport {
                    // Item below viewport - scroll down to show it
                    self.scroll.offset = (y + item_h).saturating_sub(viewport);
                }
            }
        }

        self.scroll.offset != old_offset
    }
}

// =============================================================================
// VisibleItem (internal)
// =============================================================================

/// Information about a visible item for rendering.
struct VisibleItem {
    /// Index in the items array.
    index: usize,
    /// Y offset from top of content.
    y_offset: u16,
}

// =============================================================================
// List Widget
// =============================================================================

/// Typestate marker: list needs a state reference.
pub struct NeedsState;

/// Typestate marker: list has a state reference.
pub struct HasListState<'a, T: ListItem>(pub(crate) &'a State<ListState<T>>);

/// A virtualized list widget builder.
///
/// Uses typestate pattern to enforce `state()` is called before `build()`.
/// Only creates Element objects for visible items, enabling smooth scrolling
/// with 100K+ items.
///
/// # Example
///
/// ```ignore
/// // In page! macro:
/// list (state: self.files, id: "file-list")
///     style (bg: surface)
///     on_select: file_selected()
///     on_activate: file_activated()
/// ```
pub struct List<S = NeedsState> {
    state_marker: S,
    id: Option<String>,
    style: Option<Style>,
    item_style: Option<Style>,
    item_style_selected: Option<Style>,
    item_style_focused: Option<Style>,
    transitions: Option<Transitions>,
    /// Extra items to render above/below visible area (default: 5).
    buffer: usize,
    /// Whether to show scrollbar (default: true when content exceeds viewport).
    show_scrollbar: bool,
}

impl Default for List<NeedsState> {
    fn default() -> Self {
        Self::new()
    }
}

impl List<NeedsState> {
    /// Create a new list builder.
    pub fn new() -> Self {
        Self {
            state_marker: NeedsState,
            id: None,
            style: None,
            item_style: None,
            item_style_selected: None,
            item_style_focused: None,
            transitions: None,
            buffer: 5,
            show_scrollbar: true,
        }
    }

    /// Set the state reference. Required before calling `build()`.
    pub fn state<T: ListItem>(self, s: &State<ListState<T>>) -> List<HasListState<'_, T>> {
        List {
            state_marker: HasListState(s),
            id: self.id,
            style: self.style,
            item_style: self.item_style,
            item_style_selected: self.item_style_selected,
            item_style_focused: self.item_style_focused,
            transitions: self.transitions,
            buffer: self.buffer,
            show_scrollbar: self.show_scrollbar,
        }
    }
}

impl<S> List<S> {
    /// Set the list id.
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Set the list container style.
    pub fn style(mut self, s: Style) -> Self {
        self.style = Some(s);
        self
    }

    /// Set the style for each item row.
    pub fn item_style(mut self, s: Style) -> Self {
        self.item_style = Some(s);
        self
    }

    /// Set the style for selected items.
    pub fn item_style_selected(mut self, s: Style) -> Self {
        self.item_style_selected = Some(s);
        self
    }

    /// Set the style when an item is focused.
    pub fn item_style_focused(mut self, s: Style) -> Self {
        self.item_style_focused = Some(s);
        self
    }

    /// Set transitions.
    pub fn transitions(mut self, t: Transitions) -> Self {
        self.transitions = Some(t);
        self
    }

    /// Set the buffer size (extra items rendered above/below visible area).
    ///
    /// Default is 5. Higher values reduce visual artifacts during fast
    /// scrolling but increase element count.
    pub fn buffer(mut self, buffer: usize) -> Self {
        self.buffer = buffer;
        self
    }

    /// Set whether to show the scrollbar.
    ///
    /// Default is true (shows when content exceeds viewport).
    pub fn show_scrollbar(mut self, show: bool) -> Self {
        self.show_scrollbar = show;
        self
    }
}

impl<'a, T: ListItem> List<HasListState<'a, T>> {
    /// Build the list element.
    ///
    /// Creates elements only for visible items plus a buffer. Uses spacers
    /// above and below to maintain correct scroll height.
    pub fn build(self, registry: &HandlerRegistry, handlers: &WidgetHandlers) -> Element {
        let state = self.state_marker.0;
        let mut current = state.get();
        let list_id = self.id.clone().unwrap_or_else(|| "list".into());
        let content_id = format!("{}-content", list_id);

        // Process any pending scroll request
        current.process_scroll();

        // Calculate visible range
        let visible = self.calculate_visible_items(&current);

        // Create spacer for items above visible range
        let top_spacer_height = visible
            .first()
            .map(|v| v.y_offset)
            .unwrap_or(0);

        // Create elements only for visible items
        let mut children = Vec::with_capacity(visible.len() + 2);

        if top_spacer_height > 0 {
            children.push(Element::box_().height(Size::Fixed(top_spacer_height)));
        }

        for vis_item in &visible {
            let item = &current.items[vis_item.index];
            let row = self.build_item_row(
                item,
                &current.selection,
                &list_id,
                registry,
                handlers,
                state,
            );
            children.push(row);
        }

        // Bottom spacer to maintain scroll height
        let bottom_y = visible
            .last()
            .map(|v| {
                let item = &current.items[v.index];
                v.y_offset + item.height()
            })
            .unwrap_or(0);
        let bottom_spacer_height = current.total_height().saturating_sub(bottom_y);

        if bottom_spacer_height > 0 {
            children.push(Element::box_().height(Size::Fixed(bottom_spacer_height)));
        }

        // Build content column (scrollable)
        let mut content = Element::col()
            .id(&content_id)
            .width(Size::Fill)
            .height(Size::Fill)
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
            registry.register(
                &content_id,
                "on_scroll",
                Arc::new(move |hx| {
                    if let Some((_, delta_y)) = hx.event().scroll_delta() {
                        state_clone.update(|s| {
                            s.scroll.scroll_by(delta_y);
                        });
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
            let scrollbar = Scrollbar::vertical()
                .id(format!("{}-scrollbar", list_id))
                .scroll_state(&current.scroll)
                .build();

            Element::row()
                .id(&list_id)
                .width(Size::Fill)
                .height(Size::Fill)
                .child(content)
                .child(scrollbar)
        } else {
            content.id(&list_id)
        }
    }

    /// Calculate which items are visible given current scroll state.
    fn calculate_visible_items(&self, state: &ListState<T>) -> Vec<VisibleItem> {
        let scroll_y = state.scroll.offset;
        let viewport = state.scroll.viewport;
        let buffer = self.buffer;

        if state.items.is_empty() {
            return Vec::new();
        }

        // If viewport is 0 (first frame before layout), use a safe maximum
        let effective_viewport = if viewport == 0 { 200 } else { viewport };

        // O(log n) binary search to find first visible item
        let first_visible = state.item_at_offset(scroll_y);
        let start_idx = first_visible.saturating_sub(buffer);

        // O(log n) binary search to find last visible item
        let last_visible = state.item_at_offset(scroll_y.saturating_add(effective_viewport));
        let end_idx = (last_visible + buffer + 1).min(state.items.len());

        // Collect visible items with O(1) position lookups
        let mut items = Vec::with_capacity(end_idx - start_idx);
        for i in start_idx..end_idx {
            items.push(VisibleItem {
                index: i,
                y_offset: state.item_y_offset(i),
            });
        }

        items
    }

    /// Build a single item row element.
    fn build_item_row(
        &self,
        item: &T,
        selection: &Selection<T::Key>,
        list_id: &str,
        registry: &HandlerRegistry,
        handlers: &WidgetHandlers,
        state: &State<ListState<T>>,
    ) -> Element {
        let key = item.key();
        let row_id = format!("{}-item-{}", list_id, key.to_string());
        let is_selected = selection.is_selected(&key);

        // Build row element with item's rendered content
        let mut row = Element::row()
            .id(&row_id)
            .width(Size::Fill)
            .focusable(true)
            .clickable(true)
            .child(item.render());

        // Apply base item style
        if let Some(ref style) = self.item_style {
            row = row.style(style.clone());
        }

        // Apply selected style (overrides base)
        if is_selected {
            if let Some(ref style) = self.item_style_selected {
                row = row.style(style.clone());
            } else {
                row = row.style(
                    Style::new()
                        .background(Color::var("list.item_selected"))
                        .foreground(Color::var("text.inverted")),
                );
            }
        }

        // Apply focused style
        if let Some(ref style) = self.item_style_focused {
            row = row.style_focused(style.clone());
        } else {
            row = row.style_focused(
                Style::new()
                    .background(Color::var("list.item_focused"))
                    .foreground(Color::var("text.inverted")),
            );
        }

        // Set explicit height for virtualization
        row = row.height(Size::Fixed(item.height()));

        // Register activation handler
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

        row
    }
}
