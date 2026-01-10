//! List widget - a virtualized selectable list of items.
//!
//! This widget only creates Element objects for visible items, enabling
//! smooth scrolling with 100K+ items.

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

    /// Scrollbar screen rect (x, y, width, height) for drag calculations.
    /// Updated via on_layout handler.
    scrollbar_rect: Option<(u16, u16, u16, u16)>,

    /// Grab offset within thumb for smooth dragging.
    /// Set on click, cleared on release.
    drag_grab_offset: Option<u16>,
}

impl<T: ListItem> Default for ListState<T> {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            selection: Selection::none(),
            scroll: ScrollState::new(),
            last_activated: None,
            cumulative_heights: vec![0],
            scrollbar_rect: None,
            drag_grab_offset: None,
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
            scrollbar_rect: None,
            drag_grab_offset: None,
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

    /// Get the index of the first visible item based on current scroll offset.
    pub fn first_visible_index(&self) -> usize {
        self.item_at_offset(self.scroll.offset)
    }
}

// Implement ScrollableWidgetState for ListState to use shared scrollbar handlers
impl<T: ListItem> ScrollableWidgetState for ListState<T> {
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
// VisibleItem (internal)
// =============================================================================

/// Information about a visible item for rendering.
struct VisibleItem {
    /// Index in the items array.
    index: usize,
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
    /// Whether to enable horizontal scrolling (default: false).
    /// When enabled, items can overflow horizontally and a scrollbar appears.
    horizontal_scroll: bool,
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
            horizontal_scroll: false,
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
            horizontal_scroll: self.horizontal_scroll,
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

    /// Enable horizontal scrolling.
    ///
    /// When enabled, items can overflow horizontally and a horizontal
    /// scrollbar appears when content is wider than the viewport.
    /// Default is false (items are clipped).
    pub fn horizontal_scroll(mut self, enable: bool) -> Self {
        self.horizontal_scroll = enable;
        self
    }
}

impl<'a, T: ListItem> List<HasListState<'a, T>> {
    /// Build the list element.
    ///
    /// Creates elements only for visible items plus a buffer. Uses spacers
    /// above and below to maintain correct scroll height.
    pub fn build(self, registry: &HandlerRegistry, handlers: &WidgetHandlers) -> Element {
        log::debug!("[List::build] Starting build");
        let state = self.state_marker.0;
        let list_id = self.id.clone().unwrap_or_else(|| "list".into());
        let content_id = format!("{}-content", list_id);

        // Process any pending scroll request and persist it
        state.update(|s| { s.process_scroll(); });

        let current = state.get();
        log::debug!("[List::build] Got state, items={}", current.items.len());

        // Calculate visible range
        log::debug!("[List::build] Calculating visible items");
        let visible = self.calculate_visible_items(&current);
        log::debug!("[List::build] Visible items: {}", visible.len());

        // Create elements only for visible items (no spacers - we use ScrollState)
        let mut children = Vec::with_capacity(visible.len());

        log::debug!("[List::build] Building {} item rows", visible.len());
        let visible_count = visible.len();
        let total_items = current.items.len();
        for (pos_in_visible, vis_item) in visible.iter().enumerate() {
            let item = &current.items[vis_item.index];
            let row = self.build_item_row(
                item,
                vis_item.index,
                pos_in_visible,
                visible_count,
                total_items,
                &current.selection,
                &list_id,
                registry,
                handlers,
                state,
            );
            children.push(row);
        }
        log::debug!("[List::build] Item rows built");

        // NOTE: No spacers needed - we handle scroll position via ScrollState,
        // not tuidom's scroll system. Just render visible items directly.

        log::debug!("[List::build] Building content element with {} children, horizontal_scroll={}", children.len(), self.horizontal_scroll);
        // Build content column:
        // - overflow_x: Auto (if horizontal_scroll enabled) or Hidden (default)
        // - overflow_y: Hidden - we handle vertical via virtualization (clips, no tuidom scrollbar)
        // - scrollable: true - receive scroll events for vertical scrolling
        let overflow_x = if self.horizontal_scroll {
            Overflow::Auto
        } else {
            Overflow::Hidden
        };
        let mut content = Element::col()
            .id(&content_id)
            .width(Size::Fill)
            .height(Size::Fill)
            .overflow_x(overflow_x)
            .overflow_y(Overflow::Hidden)
            .scrollable(true)
            .children(children);
        log::debug!("[List::build] Content element built");

        if let Some(ref style) = self.style {
            content = content.style(style.clone());
        }
        if let Some(ref transitions) = self.transitions {
            content = content.transitions(transitions.clone());
        }

        log::debug!("[List::build] Registering on_scroll handler");
        // Register on_scroll handler (handles both mouse wheel and keyboard scroll)
        {
            let state_clone = state.clone();
            let list_id_clone = list_id.clone();
            registry.register(
                &content_id,
                "on_scroll",
                Arc::new(move |hx| {
                    // Mouse wheel: delta
                    if let Some((_, delta_y)) = hx.event().scroll_delta() {
                        log::debug!("[List::on_scroll] scroll_delta delta_y={}", delta_y);
                        state_clone.update(|s| {
                            s.scroll.scroll_by(delta_y);
                        });
                    }
                    // Page Up/Down/Home/End: scroll action from keyboard
                    if let Some(action) = hx.event().scroll_action() {
                        log::debug!("[List::on_scroll] scroll_action {:?}", action);

                        // Apply the scroll action immediately
                        state_clone.update(|s| {
                            let scroll_request = match action {
                                tuidom::ScrollAction::PageUp => super::scroll::ScrollRequest::PageUp,
                                tuidom::ScrollAction::PageDown => super::scroll::ScrollRequest::PageDown,
                                tuidom::ScrollAction::Home => super::scroll::ScrollRequest::Home,
                                tuidom::ScrollAction::End => super::scroll::ScrollRequest::End,
                            };
                            s.scroll.apply_request(scroll_request);
                        });

                        // Calculate target based on NEW scroll position and focus it
                        let current = state_clone.get();
                        if current.items.is_empty() {
                            return;
                        }

                        let target_index = match action {
                            tuidom::ScrollAction::Home => 0,
                            tuidom::ScrollAction::End => current.items.len() - 1,
                            tuidom::ScrollAction::PageUp => {
                                // Focus first visible item after scroll
                                current.first_visible_index()
                            }
                            tuidom::ScrollAction::PageDown => {
                                // Focus last visible item after scroll
                                let first = current.first_visible_index();
                                let viewport = current.scroll.viewport as usize;
                                (first + viewport.saturating_sub(1)).min(current.items.len() - 1)
                            }
                        };

                        if let Some(item) = current.items.get(target_index) {
                            let item_id = format!("{}-item-{}", list_id_clone, item.key().to_string());
                            log::debug!("[List::on_scroll] Focusing item: {} (index {})", item_id, target_index);
                            hx.cx().focus(&item_id);
                        }
                    }
                }),
            );
        }

        log::debug!("[List::build] Registering on_layout handler");
        // Register on_layout handler for viewport discovery
        {
            let state_clone = state.clone();
            let has_horizontal_scroll = self.horizontal_scroll;
            registry.register(
                &content_id,
                "on_layout",
                Arc::new(move |hx| {
                    if let Some((_, _, _, height)) = hx.event().layout() {
                        // Subtract 1 for horizontal scrollbar only when horizontal_scroll is enabled
                        // (horizontal scrollbar takes 1 row at bottom when content overflows)
                        let viewport_height = if has_horizontal_scroll {
                            height.saturating_sub(1)
                        } else {
                            height
                        };
                        state_clone.update(|s| {
                            s.scroll.set_viewport(viewport_height);
                        });
                    }
                }),
            );
        }

        log::debug!("[List::build] Checking scrollbar");
        // Add scrollbar if needed
        let show_scrollbar = self.show_scrollbar && current.scroll.can_scroll();
        log::debug!("[List::build] show_scrollbar={}", show_scrollbar);
        if show_scrollbar {
            let scrollbar_id = format!("{}-scrollbar", list_id);
            let scrollbar = Scrollbar::vertical()
                .id(&scrollbar_id)
                .scroll_state(&current.scroll)
                .build();

            // Register scrollbar handlers for click/drag scrolling
            register_scrollbar_handlers(&scrollbar_id, registry, state);

            log::debug!("[List::build] Build complete (with scrollbar)");
            Element::row()
                .id(&list_id)
                .width(Size::Fill)
                .height(Size::Fill)
                .child(content)
                .child(scrollbar)
        } else {
            log::debug!("[List::build] Build complete (no scrollbar)");
            content.id(&list_id)
        }
    }

    /// Calculate which items are visible given current scroll state.
    fn calculate_visible_items(&self, state: &ListState<T>) -> Vec<VisibleItem> {
        let scroll_y = state.scroll.offset;
        let viewport = state.scroll.viewport;

        if state.items.is_empty() {
            return Vec::new();
        }

        // If viewport is 0 (first frame before layout), use a safe maximum
        let effective_viewport = if viewport == 0 { 200 } else { viewport };

        // O(log n) binary search to find first visible item
        let first_visible = state.item_at_offset(scroll_y);

        // Calculate exactly how many items fit in viewport
        let mut end_idx = first_visible;
        let mut total_height: u16 = 0;
        while end_idx < state.items.len() && total_height < effective_viewport {
            total_height += state.item_height(end_idx);
            end_idx += 1;
        }

        // Collect visible items
        let mut items = Vec::with_capacity(end_idx - first_visible);
        for i in first_visible..end_idx {
            items.push(VisibleItem { index: i });
        }

        items
    }

    /// Build a single item row element.
    fn build_item_row(
        &self,
        item: &T,
        item_index: usize,
        pos_in_visible: usize,
        visible_count: usize,
        total_items: usize,
        selection: &Selection<T::Key>,
        list_id: &str,
        registry: &HandlerRegistry,
        handlers: &WidgetHandlers,
        state: &State<ListState<T>>,
    ) -> Element {
        let key = item.key();
        let row_id = format!("{}-item-{}", list_id, key.to_string());
        let is_selected = selection.is_selected(&key);

        // Check if this item is at a scroll boundary
        let is_at_top_boundary = pos_in_visible == 0 && item_index > 0;
        let is_at_bottom_boundary = pos_in_visible == visible_count - 1 && item_index < total_items - 1;

        // Build row element with item's rendered content
        // Use Auto width for horizontal scrolling, Fill otherwise
        let row_width = if self.horizontal_scroll {
            Size::Auto
        } else {
            Size::Fill
        };
        let mut row = Element::row()
            .id(&row_id)
            .width(row_width)
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

        // Register key handler for boundary scrolling
        // When Up/Down is pressed at a boundary, scroll and focus the next item
        if is_at_top_boundary {
            let state_clone = state.clone();
            let list_id_clone = list_id.to_string();
            let target_index = item_index.saturating_sub(1);
            registry.register(
                &row_id,
                "on_key_up",
                Arc::new(move |hx| {
                    log::debug!("[List::on_key_up] at top boundary, scrolling up");
                    state_clone.update(|s| {
                        s.scroll.apply_request(super::scroll::ScrollRequest::Delta(-1));
                    });
                    // Focus the previous item
                    let current = state_clone.get();
                    if let Some(item) = current.items.get(target_index) {
                        let item_id = format!("{}-item-{}", list_id_clone, item.key().to_string());
                        log::debug!("[List::on_key_up] Focusing item: {}", item_id);
                        hx.cx().focus(&item_id);
                    }
                }),
            );
        }
        if is_at_bottom_boundary {
            let state_clone = state.clone();
            let list_id_clone = list_id.to_string();
            let target_index = item_index + 1;
            registry.register(
                &row_id,
                "on_key_down",
                Arc::new(move |hx| {
                    log::debug!("[List::on_key_down] at bottom boundary, scrolling down");
                    state_clone.update(|s| {
                        s.scroll.apply_request(super::scroll::ScrollRequest::Delta(1));
                    });
                    // Focus the next item
                    let current = state_clone.get();
                    if let Some(item) = current.items.get(target_index) {
                        let item_id = format!("{}-item-{}", list_id_clone, item.key().to_string());
                        log::debug!("[List::on_key_down] Focusing item: {}", item_id);
                        hx.cx().focus(&item_id);
                    }
                }),
            );
        }

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
