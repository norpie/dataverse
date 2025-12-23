//! Shared traits for scrollable widgets.
//!
//! These traits define the common interface for widgets that support
//! scrolling and share the same state management patterns.

use crate::context::AppContext;
use crate::input::keybinds::{Key, KeyCombo};

use super::events::{EventResult, WidgetEvent, WidgetEventKind};
use super::scrollbar::ScrollbarState;
use super::selection::SelectionMode;

/// Trait for widgets that support scrollable content.
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
/// impl ScrollableWidget for MyComponent {
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
pub trait ScrollableWidget: ScrollbarState {
    /// Get the unique ID as a string (for node binding).
    fn id_string(&self) -> String;

    /// Check if the widget state has changed and needs re-render.
    fn is_dirty(&self) -> bool;

    /// Clear the dirty flag after rendering.
    fn clear_dirty(&self);
}

/// Trait for widgets that support cursor navigation and selection.
///
/// This trait provides a unified interface for List, Tree, and Table widgets,
/// enabling shared event handling logic for keyboard navigation and selection.
///
/// # Trait Hierarchy
///
/// ```text
/// ScrollbarState
///     └── ScrollableWidget
///             └── SelectableWidget
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
pub trait SelectableWidget: ScrollableWidget {
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
            cx.push_event(WidgetEvent::new(
                WidgetEventKind::CursorMove,
                self.id_string(),
            ));
        }
    }

    /// Push an activate event to the context.
    fn push_activate_event(&self, cx: &AppContext) {
        if let Some(id) = self.cursor_id() {
            cx.set_activated(id, self.cursor());
            cx.push_event(WidgetEvent::new(
                WidgetEventKind::Activate,
                self.id_string(),
            ));
        }
    }

    /// Push a selection change event to the context.
    fn push_selection_event(&self, added: &[String], removed: &[String], cx: &AppContext) {
        if !added.is_empty() || !removed.is_empty() {
            cx.set_selected(self.selected_ids());
            cx.push_event(WidgetEvent::new(
                WidgetEventKind::SelectionChange,
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
    /// Override this for widgets with headers (like Table) that need
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
            Key::Char('a')
                if key.modifiers.ctrl && self.selection_mode() == SelectionMode::Multiple =>
            {
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

// =============================================================================
// AnySelectable - Type-erased interface for selectable widgets
// =============================================================================

/// Unified interface for type-erased selectable widgets (List, Tree, Table).
///
/// This trait provides a common interface for the event loop to handle
/// click events polymorphically without branching on widget type.
///
/// Unlike `SelectableWidget` which is implemented on concrete generic types,
/// this trait is object-safe and used for dynamic dispatch.
///
/// # Usage
///
/// The event loop uses this trait to handle clicks on any selectable widget:
///
/// ```ignore
/// if let Some(selectable) = page.get_selectable_component(&id) {
///     if selectable.has_header() && y == 0 {
///         selectable.on_header_click(x, cx);
///     } else {
///         selectable.on_click_with_modifiers(y, ctrl, shift, cx);
///     }
/// }
/// ```
pub trait AnySelectable: Send + Sync {
    /// Get the widget ID.
    fn id_string(&self) -> String;

    /// Handle click with modifiers on a data row.
    ///
    /// For Table, `y_in_viewport` includes the header row (y=0 is header, y=1+ is data).
    /// The implementation should handle this offset internally.
    fn on_click_with_modifiers(
        &self,
        y_in_viewport: u16,
        ctrl: bool,
        shift: bool,
        cx: &AppContext,
    ) -> EventResult;

    /// Whether this widget has a header row (Table only).
    fn has_header(&self) -> bool {
        false
    }

    /// Handle header click (Table only).
    ///
    /// `x_in_viewport` is the x coordinate relative to the widget's left edge.
    fn on_header_click(&self, _x_in_viewport: u16, _cx: &AppContext) -> EventResult {
        EventResult::Ignored
    }
}

// =============================================================================
// New Unified Widget Trait Hierarchy
// =============================================================================

use ratatui::Frame;
use ratatui::layout::Rect;
use std::fmt::Debug;

use crate::input::events::Modifiers;
use crate::input::keybinds::HandlerId;
use crate::layers::overlay::OverlayRequest;
use crate::node::Node;
use crate::runtime::hit_test::HitTestMap;
use crate::styling::theme::Theme;

/// Context passed to widgets during rendering.
///
/// This provides widgets with access to runtime resources needed for rendering,
/// including theme colors, hit testing registration, and recursive node rendering.
pub struct RenderContext<'a> {
    /// Theme for resolving colors
    pub theme: &'a dyn Theme,
    /// Hit test map for registering clickable areas
    pub hit_map: &'a mut HitTestMap,
    /// Function for rendering child nodes (used by container widgets)
    pub render_node: fn(
        &mut Frame,
        &Node,
        Rect,
        &mut HitTestMap,
        &dyn Theme,
        Option<&str>,
        &mut Vec<OverlayRequest>,
    ),
    /// ID of the currently focused widget
    pub focused_id: Option<&'a str>,
    /// Pre-resolved ratatui style for this widget
    pub style: ratatui::style::Style,
    /// Layout configuration for this widget
    pub layout: &'a crate::node::Layout,
    /// Child nodes (for container widgets like ScrollArea)
    pub children: &'a [Node],
    /// Overlay requests (widgets register overlays here during render)
    pub overlay_requests: &'a mut Vec<OverlayRequest>,
}

impl<'a> RenderContext<'a> {
    /// Register an overlay to be rendered in the overlay layer.
    ///
    /// Widgets call this during their render phase when they need to display
    /// floating content (dropdowns, menus, tooltips, etc.).
    ///
    /// # Example
    ///
    /// ```ignore
    /// if self.is_open() {
    ///     ctx.register_overlay(OverlayRequest {
    ///         owner_id: self.id_string(),
    ///         content: build_dropdown_content(),
    ///         anchor: area,
    ///         position: OverlayPosition::Below,
    ///     });
    /// }
    /// ```
    pub fn register_overlay(&mut self, request: OverlayRequest) {
        self.overlay_requests.push(request);
    }
}

/// Handler composition for widgets.
///
/// This struct holds all possible handlers that can be attached to a widget.
/// Any handler can be set on any widget - if a widget doesn't support a particular
/// event type, the handler will simply never be called (similar to HTML event handlers).
#[derive(Debug, Default, Clone)]
pub struct WidgetHandlers {
    /// Click handler (for buttons, interactive elements)
    pub on_click: Option<HandlerId>,
    /// Activation handler (Enter key, double-click on list items, etc.)
    pub on_activate: Option<HandlerId>,
    /// Change handler (input value changed, checkbox toggled, etc.)
    pub on_change: Option<HandlerId>,
    /// Submit handler (Enter on input, form submission)
    pub on_submit: Option<HandlerId>,
    /// Cursor movement handler (list/tree/table cursor moved)
    pub on_cursor_move: Option<HandlerId>,
    /// Selection change handler (list/tree/table selection changed)
    pub on_selection_change: Option<HandlerId>,
    /// Scroll handler (scrollable content was scrolled)
    pub on_scroll: Option<HandlerId>,
    /// Expand handler (tree node expanded)
    pub on_expand: Option<HandlerId>,
    /// Collapse handler (tree node collapsed)
    pub on_collapse: Option<HandlerId>,
    /// Sort handler (table column sorted)
    pub on_sort: Option<HandlerId>,
}

impl WidgetHandlers {
    /// Create empty handlers
    pub fn new() -> Self {
        Self::default()
    }

    /// Get handler for a given event kind
    pub fn get(&self, kind: WidgetEventKind) -> Option<&HandlerId> {
        match kind {
            WidgetEventKind::Activate => self.on_activate.as_ref(),
            WidgetEventKind::CursorMove => self.on_cursor_move.as_ref(),
            WidgetEventKind::SelectionChange => self.on_selection_change.as_ref(),
            WidgetEventKind::Expand => self.on_expand.as_ref(),
            WidgetEventKind::Collapse => self.on_collapse.as_ref(),
            WidgetEventKind::Sort => self.on_sort.as_ref(),
            WidgetEventKind::Change => self.on_change.as_ref(),
        }
    }
}

/// Base trait for all widgets.
///
/// This is the unified interface that all widgets implement, enabling:
/// - Storage in `Node::Widget` via `Box<dyn AnyWidget>`
/// - Polymorphic event dispatch
/// - Unified rendering
/// - Capability queries for scroll/selection features
///
/// # Object Safety
///
/// This trait is designed to be object-safe, allowing widgets to be stored
/// as trait objects. All methods either take `&self` or return types that
/// don't prevent object safety.
///
/// # Implementing for External Widgets
///
/// External crates can implement this trait to create custom widgets:
///
/// ```ignore
/// impl AnyWidget for MyCustomWidget {
///     fn id(&self) -> String {
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
///
///     fn render(&self, frame: &mut Frame, area: Rect, focused: bool, ctx: &mut RenderContext<'_>) {
///         // Render using ratatui primitives
///         // Access theme colors via ctx.theme
///         // Register hit areas via ctx.hit_map
///     }
/// }
/// ```
pub trait AnyWidget: Send + Sync + Debug {
    /// Get the unique identifier for this widget instance.
    fn id(&self) -> String;

    /// Check if the widget's state has changed since the last render.
    fn is_dirty(&self) -> bool;

    /// Clear the dirty flag after rendering.
    fn clear_dirty(&self);

    /// Check if this widget can receive keyboard focus.
    fn is_focusable(&self) -> bool {
        true
    }

    /// Check if this widget captures text input when focused.
    ///
    /// When true, keyboard events (except Tab/Escape) are sent to this widget
    /// instead of being processed as keybinds.
    fn captures_input(&self) -> bool {
        false
    }

    // =========================================================================
    // Layout
    // =========================================================================

    /// Get the intrinsic width of this widget (in columns).
    ///
    /// Used for auto-sizing when no explicit width is specified.
    /// Default returns 0, meaning the widget will use available space.
    fn intrinsic_width(&self) -> u16 {
        0
    }

    /// Get the intrinsic height of this widget (in rows).
    ///
    /// Used for auto-sizing when no explicit height is specified.
    /// Default returns 1, meaning the widget takes one row.
    fn intrinsic_height(&self) -> u16 {
        1
    }

    /// Whether this widget stacks children vertically (adding heights).
    ///
    /// When true, the layout system calculates intrinsic height as
    /// `widget_height + children_height` (stacking). When false, it uses
    /// `max(widget_height, children_height)` (overlay/scroll).
    ///
    /// Default is false (overlay behavior, like ScrollArea).
    /// Collapsible returns true because header + content are stacked.
    fn stacks_children(&self) -> bool {
        false
    }

    /// Whether this widget handles its own hit area registration.
    ///
    /// When true, the render system will NOT register the full widget area
    /// as a hit box after rendering. The widget is responsible for calling
    /// `ctx.hit_map.register()` for its clickable regions.
    ///
    /// This is useful for container widgets like Collapsible where only
    /// the header should be clickable (for the widget itself), and children
    /// should receive their own click events.
    ///
    /// Default is false (full area is registered automatically).
    fn registers_own_hit_area(&self) -> bool {
        false
    }

    /// Whether this widget's children should be excluded from layout calculations.
    ///
    /// When true, the layout system ignores children when calculating the widget's
    /// intrinsic size. This is useful for widgets like Select where children
    /// are stored for overlay content but shouldn't affect the trigger's size.
    ///
    /// Default is false (children contribute to intrinsic size).
    fn hides_children_from_layout(&self) -> bool {
        false
    }

    // =========================================================================
    // Event Dispatch
    // =========================================================================

    /// Handle a click event at the given position.
    ///
    /// Position is relative to the widget's top-left corner.
    /// Return `EventResult::StartDrag` to begin a drag operation.
    fn dispatch_click(&self, _x: u16, _y: u16, _cx: &AppContext) -> EventResult {
        EventResult::Ignored
    }

    /// Handle a key event when this widget is focused.
    fn dispatch_key(&self, _key: &KeyCombo, _cx: &AppContext) -> EventResult {
        EventResult::Ignored
    }

    /// Handle a hover event at the given position.
    fn dispatch_hover(&self, _x: u16, _y: u16, _cx: &AppContext) -> EventResult {
        EventResult::Ignored
    }

    /// Handle a scroll event.
    fn dispatch_scroll(
        &self,
        _direction: crate::input::events::ScrollDirection,
        _amount: u16,
        _cx: &AppContext,
    ) -> EventResult {
        EventResult::Ignored
    }

    /// Handle ongoing drag movement.
    fn dispatch_drag(
        &self,
        _x: u16,
        _y: u16,
        _modifiers: Modifiers,
        _cx: &AppContext,
    ) -> EventResult {
        EventResult::Ignored
    }

    /// Handle drag release.
    fn dispatch_release(&self, _cx: &AppContext) -> EventResult {
        EventResult::Ignored
    }

    /// Handle focus loss (blur).
    ///
    /// Called when this widget loses focus. Useful for widgets that need
    /// to perform cleanup when focus moves away, such as closing overlays.
    ///
    /// # Example
    ///
    /// ```ignore
    /// fn dispatch_blur(&self, _cx: &AppContext) {
    ///     // Close any open overlay when focus leaves
    ///     self.close();
    /// }
    /// ```
    fn dispatch_blur(&self, _cx: &AppContext) {
        // Default: do nothing
    }

    /// Handle a click event inside an overlay owned by this widget.
    ///
    /// This is called when the user clicks inside the overlay area.
    /// The coordinates are relative to the overlay's top-left corner.
    ///
    /// # Arguments
    ///
    /// * `x` - X coordinate relative to overlay's left edge
    /// * `y` - Y coordinate relative to overlay's top edge
    /// * `cx` - Application context
    ///
    /// # Returns
    ///
    /// `EventResult::Consumed` if the click was handled, `EventResult::Ignored` otherwise.
    fn dispatch_overlay_click(&self, _x: u16, _y: u16, _cx: &AppContext) -> EventResult {
        EventResult::Ignored
    }

    /// Handle a scroll event inside an overlay owned by this widget.
    ///
    /// This is called when the user scrolls inside the overlay area.
    ///
    /// # Arguments
    ///
    /// * `direction` - Scroll direction
    /// * `amount` - Scroll amount
    /// * `cx` - Application context
    ///
    /// # Returns
    ///
    /// `EventResult::Consumed` if the scroll was handled, `EventResult::Ignored` otherwise.
    fn dispatch_overlay_scroll(
        &self,
        _direction: crate::input::events::ScrollDirection,
        _amount: u16,
        _cx: &AppContext,
    ) -> EventResult {
        EventResult::Ignored
    }

    /// Handle a hover event inside an overlay owned by this widget.
    ///
    /// This is called when the mouse moves inside the overlay area.
    /// The coordinates are relative to the overlay's top-left corner.
    ///
    /// # Arguments
    ///
    /// * `x` - X coordinate relative to overlay's left edge
    /// * `y` - Y coordinate relative to overlay's top edge
    /// * `cx` - Application context
    ///
    /// # Returns
    ///
    /// `EventResult::Consumed` if the hover was handled, `EventResult::Ignored` otherwise.
    fn dispatch_overlay_hover(&self, _x: u16, _y: u16, _cx: &AppContext) -> EventResult {
        EventResult::Ignored
    }

    // =========================================================================
    // Rendering
    // =========================================================================

    /// Render the widget to the frame.
    ///
    /// # Arguments
    ///
    /// * `frame` - The ratatui frame to render to
    /// * `area` - The rectangular area allocated for this widget
    /// * `focused` - Whether this widget currently has keyboard focus
    /// * `ctx` - Render context with theme, hit_map, and render_node function
    fn render(&self, frame: &mut Frame, area: Rect, focused: bool, ctx: &mut RenderContext<'_>);

    // =========================================================================
    // Capability Queries
    // =========================================================================

    /// Get scrollable capability if this widget supports scrolling.
    ///
    /// Returns `Some(&dyn Scrollable)` if the widget can be scrolled.
    fn as_scrollable(&self) -> Option<&dyn Scrollable> {
        None
    }

    /// Get selectable capability if this widget supports selection.
    ///
    /// Returns `Some(&dyn Selectable)` if the widget has cursor/selection.
    fn as_selectable(&self) -> Option<&dyn Selectable> {
        None
    }
}

/// Scrollable widget capability.
///
/// Widgets that support scrolling implement this trait to expose their
/// scroll state and allow programmatic scrolling.
pub trait Scrollable: Send + Sync {
    /// Get the current vertical scroll offset.
    fn scroll_offset(&self) -> usize;

    /// Set the vertical scroll offset.
    fn set_scroll_offset(&self, offset: usize);

    /// Get the viewport size (visible area).
    fn viewport_size(&self) -> usize;

    /// Get the total content size.
    fn content_size(&self) -> usize;

    /// Scroll to a specific position.
    fn scroll_to(&self, position: usize) {
        self.set_scroll_offset(position);
    }

    /// Get the maximum scroll offset.
    fn max_scroll(&self) -> usize {
        self.content_size().saturating_sub(self.viewport_size())
    }

    /// Check if scrolling is needed.
    fn needs_scroll(&self) -> bool {
        self.content_size() > self.viewport_size()
    }
}

/// Selectable widget capability.
///
/// Widgets that support cursor navigation and item selection implement this trait.
/// This is typically used by List, Tree, and Table widgets.
///
/// Note: This trait extends `Scrollable` because selectable widgets typically
/// need to scroll to keep the cursor visible.
pub trait Selectable: Scrollable {
    /// Get the current cursor position (item index).
    fn cursor(&self) -> Option<usize>;

    /// Set the cursor position. Returns the previous position.
    fn set_cursor(&self, index: usize) -> Option<usize>;

    /// Get the ID of the item at the cursor position.
    fn cursor_id(&self) -> Option<String>;

    /// Move cursor up. Returns `(previous, new)` if moved.
    fn cursor_up(&self) -> Option<(Option<usize>, usize)>;

    /// Move cursor down. Returns `(previous, new)` if moved.
    fn cursor_down(&self) -> Option<(Option<usize>, usize)>;

    /// Move cursor to first item. Returns `(previous, new)` if moved.
    fn cursor_first(&self) -> Option<(Option<usize>, usize)>;

    /// Move cursor to last item. Returns `(previous, new)` if moved.
    fn cursor_last(&self) -> Option<(Option<usize>, usize)>;

    /// Scroll the viewport to make the cursor visible.
    fn scroll_to_cursor(&self);

    /// Get the selection mode.
    fn selection_mode(&self) -> SelectionMode;

    /// Get all selected item IDs.
    fn selected_ids(&self) -> Vec<String>;

    /// Toggle selection of the item at cursor.
    /// Returns (added IDs, removed IDs).
    fn toggle_select_at_cursor(&self) -> (Vec<String>, Vec<String>);

    /// Select all items. Returns newly selected IDs.
    fn select_all(&self) -> Vec<String>;

    /// Clear all selection. Returns deselected IDs.
    fn deselect_all(&self) -> Vec<String>;

    /// Get the total number of items.
    fn item_count(&self) -> usize;

    /// Get the number of items that fit in the viewport.
    fn viewport_item_count(&self) -> usize;

    /// Get the height of a single item (in rows).
    fn item_height(&self) -> u16;

    // =========================================================================
    // Click Handling (with defaults for non-Table widgets)
    // =========================================================================

    /// Whether this widget has a header row (Table only).
    fn has_header(&self) -> bool {
        false
    }

    /// Handle header click (Table only).
    ///
    /// `x_in_viewport` is the x coordinate relative to the widget's left edge.
    fn on_header_click(&self, _x_in_viewport: u16, _cx: &AppContext) -> EventResult {
        EventResult::Ignored
    }

    /// Handle click with modifiers on a data row.
    ///
    /// For Table, `y_in_viewport` includes the header row offset.
    /// The implementation should handle this internally.
    fn on_click_with_modifiers(
        &self,
        _y_in_viewport: u16,
        _ctrl: bool,
        _shift: bool,
        _cx: &AppContext,
    ) -> EventResult {
        EventResult::Ignored
    }
}
