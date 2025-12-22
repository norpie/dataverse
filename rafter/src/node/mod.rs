//! Node types for the page tree.

mod layout;

pub use layout::{Align, Border, Direction, Justify, Layout, Size};

use crate::context::AppContext;
use crate::input::events::{Modifiers, ScrollDirection};
use crate::input::keybinds::{HandlerId, KeyCombo};
use crate::styling::style::Style;
use crate::widgets::events::EventResult;
use crate::widgets::{AnyWidget, WidgetHandlers};

/// A node in the page tree.
///
/// Nodes are either primitives (layout containers and text) or widgets
/// (interactive elements with state).
#[derive(Debug, Default)]
pub enum Node {
    // =========================================================================
    // Primitives (stateless layout nodes)
    // =========================================================================

    /// Empty node (renders nothing)
    #[default]
    Empty,

    /// Text content
    Text {
        content: String,
        style: Style,
    },

    /// Container with vertical layout
    Column {
        children: Vec<Node>,
        style: Style,
        layout: Layout,
    },

    /// Container with horizontal layout
    Row {
        children: Vec<Node>,
        style: Style,
        layout: Layout,
    },

    /// Stack (z-axis layering)
    Stack {
        children: Vec<Node>,
        style: Style,
        layout: Layout,
    },

    // =========================================================================
    // Unified Widget variant (for widgets implementing AnyWidget)
    // =========================================================================

    /// A widget node.
    ///
    /// This is the unified variant for all widgets implementing `AnyWidget`.
    /// It holds the widget as a trait object along with its handlers.
    /// Container widgets (like ScrollArea) use the `children` field for slot content.
    Widget {
        /// The widget instance (type-erased)
        widget: Box<dyn AnyWidget>,
        /// Event handlers for this widget
        handlers: WidgetHandlers,
        /// Style
        style: Style,
        /// Layout properties
        layout: Layout,
        /// Child nodes (for container widgets like ScrollArea)
        children: Vec<Node>,
    },
}

impl Node {
    // =========================================================================
    // Constructors
    // =========================================================================

    /// Create an empty node
    pub const fn empty() -> Self {
        Self::Empty
    }

    /// Create a text node
    pub fn text(content: impl Into<String>) -> Self {
        Self::Text {
            content: content.into(),
            style: Style::new(),
        }
    }

    /// Create a text node with style
    pub fn text_styled(content: impl Into<String>, style: Style) -> Self {
        Self::Text {
            content: content.into(),
            style,
        }
    }

    /// Create a column node
    pub fn column(children: Vec<Node>) -> Self {
        Self::Column {
            children,
            style: Style::new(),
            layout: Layout::default(),
        }
    }

    /// Create a column node with style and layout
    pub fn column_styled(children: Vec<Node>, style: Style, layout: Layout) -> Self {
        Self::Column {
            children,
            style,
            layout,
        }
    }

    /// Create a row node
    pub fn row(children: Vec<Node>) -> Self {
        Self::Row {
            children,
            style: Style::new(),
            layout: Layout::default(),
        }
    }

    /// Create a row node with style and layout
    pub fn row_styled(children: Vec<Node>, style: Style, layout: Layout) -> Self {
        Self::Row {
            children,
            style,
            layout,
        }
    }

    /// Check if node is empty
    pub fn is_empty(&self) -> bool {
        matches!(self, Self::Empty)
    }

    /// Create a widget node
    pub fn widget(
        widget: Box<dyn AnyWidget>,
        handlers: WidgetHandlers,
        style: Style,
        layout: Layout,
    ) -> Self {
        Self::Widget {
            widget,
            handlers,
            style,
            layout,
            children: Vec::new(),
        }
    }

    /// Create a container widget node with children
    pub fn container_widget(
        widget: Box<dyn AnyWidget>,
        handlers: WidgetHandlers,
        style: Style,
        layout: Layout,
        children: Vec<Node>,
    ) -> Self {
        Self::Widget {
            widget,
            handlers,
            style,
            layout,
            children,
        }
    }

    // =========================================================================
    // Node Properties
    // =========================================================================

    /// Check if this node is focusable
    pub fn is_focusable(&self) -> bool {
        match self {
            Self::Widget { widget, .. } => widget.is_focusable(),
            _ => false,
        }
    }

    /// Get the element ID if any
    pub fn id(&self) -> Option<&str> {
        match self {
            Self::Widget { widget, .. } => Some(widget.id().leak()), // TODO: avoid leak
            _ => None,
        }
    }

    /// Check if this node captures text input when focused
    pub fn captures_input(&self) -> bool {
        match self {
            Self::Widget { widget, .. } => widget.captures_input(),
            _ => false,
        }
    }

    // =========================================================================
    // Focus Management
    // =========================================================================

    /// Collect all focusable element IDs from this node and its children (in tree order)
    pub fn collect_focusable_ids(&self, ids: &mut Vec<String>) {
        match self {
            Self::Widget {
                widget, children, ..
            } => {
                if widget.is_focusable() {
                    ids.push(widget.id());
                }
                // Also collect from children (for container widgets)
                for child in children {
                    child.collect_focusable_ids(ids);
                }
            }
            Self::Column { children, .. }
            | Self::Row { children, .. }
            | Self::Stack { children, .. } => {
                for child in children {
                    child.collect_focusable_ids(ids);
                }
            }
            _ => {}
        }
    }

    /// Get all focusable element IDs in tree order
    pub fn focusable_ids(&self) -> Vec<String> {
        let mut ids = Vec::new();
        self.collect_focusable_ids(&mut ids);
        ids
    }

    /// Check if an element with the given ID captures text input
    pub fn element_captures_input(&self, target_id: &str) -> bool {
        match self {
            Self::Widget {
                widget, children, ..
            } if widget.id() == target_id => widget.captures_input(),
            Self::Widget { children, .. } => {
                children.iter().any(|c| c.element_captures_input(target_id))
            }
            Self::Column { children, .. }
            | Self::Row { children, .. }
            | Self::Stack { children, .. } => {
                children.iter().any(|c| c.element_captures_input(target_id))
            }
            _ => false,
        }
    }

    // =========================================================================
    // Widget Access
    // =========================================================================

    /// Get a widget by ID.
    ///
    /// This is the unified way to access any widget implementing `AnyWidget`.
    pub fn get_widget(&self, target_id: &str) -> Option<&dyn AnyWidget> {
        match self {
            Self::Widget {
                widget, children, ..
            } => {
                if widget.id() == target_id {
                    Some(widget.as_ref())
                } else {
                    children.iter().find_map(|c| c.get_widget(target_id))
                }
            }
            Self::Column { children, .. }
            | Self::Row { children, .. }
            | Self::Stack { children, .. } => {
                children.iter().find_map(|c| c.get_widget(target_id))
            }
            _ => None,
        }
    }

    /// Get widget handlers by ID.
    pub fn get_widget_handlers(&self, target_id: &str) -> Option<&WidgetHandlers> {
        match self {
            Self::Widget {
                widget,
                handlers,
                children,
                ..
            } => {
                if widget.id() == target_id {
                    Some(handlers)
                } else {
                    children
                        .iter()
                        .find_map(|c| c.get_widget_handlers(target_id))
                }
            }
            Self::Column { children, .. }
            | Self::Row { children, .. }
            | Self::Stack { children, .. } => {
                children
                    .iter()
                    .find_map(|c| c.get_widget_handlers(target_id))
            }
            _ => None,
        }
    }

    // =========================================================================
    // Handler Getters
    // =========================================================================

    /// Get the handler for a focusable element (on_click for buttons, on_submit for inputs, on_activate for lists)
    pub fn get_submit_handler(&self, target_id: &str) -> Option<HandlerId> {
        match self {
            Self::Widget {
                widget,
                handlers,
                children,
                ..
            } => {
                if widget.id() == target_id {
                    handlers
                        .on_click
                        .clone()
                        .or_else(|| handlers.on_activate.clone())
                        .or_else(|| handlers.on_submit.clone())
                } else {
                    children
                        .iter()
                        .find_map(|c| c.get_submit_handler(target_id))
                }
            }
            Self::Column { children, .. }
            | Self::Row { children, .. }
            | Self::Stack { children, .. } => {
                children
                    .iter()
                    .find_map(|c| c.get_submit_handler(target_id))
            }
            _ => None,
        }
    }

    /// Get the on_change handler for an input or checkbox element by ID
    pub fn get_change_handler(&self, target_id: &str) -> Option<HandlerId> {
        match self {
            Self::Widget {
                widget,
                handlers,
                children,
                ..
            } => {
                if widget.id() == target_id {
                    handlers.on_change.clone()
                } else {
                    children
                        .iter()
                        .find_map(|c| c.get_change_handler(target_id))
                }
            }
            Self::Column { children, .. }
            | Self::Row { children, .. }
            | Self::Stack { children, .. } => {
                children
                    .iter()
                    .find_map(|c| c.get_change_handler(target_id))
            }
            _ => None,
        }
    }

    /// Get the on_cursor_move handler by ID.
    pub fn get_cursor_handler(&self, target_id: &str) -> Option<HandlerId> {
        match self {
            Self::Widget {
                widget,
                handlers,
                children,
                ..
            } => {
                if widget.id() == target_id {
                    handlers.on_cursor_move.clone()
                } else {
                    children
                        .iter()
                        .find_map(|c| c.get_cursor_handler(target_id))
                }
            }
            Self::Column { children, .. }
            | Self::Row { children, .. }
            | Self::Stack { children, .. } => {
                children
                    .iter()
                    .find_map(|c| c.get_cursor_handler(target_id))
            }
            _ => None,
        }
    }

    /// Get the on_selection_change handler by ID.
    pub fn get_selection_handler(&self, target_id: &str) -> Option<HandlerId> {
        match self {
            Self::Widget {
                widget,
                handlers,
                children,
                ..
            } => {
                if widget.id() == target_id {
                    handlers.on_selection_change.clone()
                } else {
                    children
                        .iter()
                        .find_map(|c| c.get_selection_handler(target_id))
                }
            }
            Self::Column { children, .. }
            | Self::Row { children, .. }
            | Self::Stack { children, .. } => {
                children
                    .iter()
                    .find_map(|c| c.get_selection_handler(target_id))
            }
            _ => None,
        }
    }

    /// Get the on_expand handler by ID.
    pub fn get_expand_handler(&self, target_id: &str) -> Option<HandlerId> {
        match self {
            Self::Widget {
                widget,
                handlers,
                children,
                ..
            } => {
                if widget.id() == target_id {
                    handlers.on_expand.clone()
                } else {
                    children
                        .iter()
                        .find_map(|c| c.get_expand_handler(target_id))
                }
            }
            Self::Column { children, .. }
            | Self::Row { children, .. }
            | Self::Stack { children, .. } => {
                children
                    .iter()
                    .find_map(|c| c.get_expand_handler(target_id))
            }
            _ => None,
        }
    }

    /// Get the on_collapse handler by ID.
    pub fn get_collapse_handler(&self, target_id: &str) -> Option<HandlerId> {
        match self {
            Self::Widget {
                widget,
                handlers,
                children,
                ..
            } => {
                if widget.id() == target_id {
                    handlers.on_collapse.clone()
                } else {
                    children
                        .iter()
                        .find_map(|c| c.get_collapse_handler(target_id))
                }
            }
            Self::Column { children, .. }
            | Self::Row { children, .. }
            | Self::Stack { children, .. } => {
                children
                    .iter()
                    .find_map(|c| c.get_collapse_handler(target_id))
            }
            _ => None,
        }
    }

    /// Get the on_sort handler by ID.
    pub fn get_sort_handler(&self, target_id: &str) -> Option<HandlerId> {
        match self {
            Self::Widget {
                widget,
                handlers,
                children,
                ..
            } => {
                if widget.id() == target_id {
                    handlers.on_sort.clone()
                } else {
                    children.iter().find_map(|c| c.get_sort_handler(target_id))
                }
            }
            Self::Column { children, .. }
            | Self::Row { children, .. }
            | Self::Stack { children, .. } => {
                children.iter().find_map(|c| c.get_sort_handler(target_id))
            }
            _ => None,
        }
    }

    /// Get the on_scroll handler by ID
    pub fn get_list_scroll_handler(&self, target_id: &str) -> Option<HandlerId> {
        match self {
            Self::Widget {
                widget,
                handlers,
                children,
                ..
            } => {
                if widget.id() == target_id {
                    handlers.on_scroll.clone()
                } else {
                    children
                        .iter()
                        .find_map(|c| c.get_list_scroll_handler(target_id))
                }
            }
            Self::Column { children, .. }
            | Self::Row { children, .. }
            | Self::Stack { children, .. } => {
                children
                    .iter()
                    .find_map(|c| c.get_list_scroll_handler(target_id))
            }
            _ => None,
        }
    }

    // =========================================================================
    // Event Dispatch
    // =========================================================================

    /// Dispatch an event to a widget by ID using a visitor function.
    fn dispatch_event<F>(&self, target_id: &str, visitor: F) -> Option<EventResult>
    where
        F: Fn(&Node) -> Option<EventResult> + Copy,
    {
        if let Some(id) = self.id()
            && id == target_id
            && let Some(result) = visitor(self)
        {
            return Some(result);
        }

        match self {
            Self::Widget { children, .. } => {
                children.iter().find_map(|c| c.dispatch_event(target_id, visitor))
            }
            Self::Column { children, .. }
            | Self::Row { children, .. }
            | Self::Stack { children, .. } => {
                children.iter().find_map(|c| c.dispatch_event(target_id, visitor))
            }
            _ => None,
        }
    }

    /// Dispatch a click event to a widget.
    pub fn dispatch_click_event(
        &self,
        target_id: &str,
        x: u16,
        y: u16,
        cx: &AppContext,
    ) -> Option<EventResult> {
        self.dispatch_event(target_id, |node| match node {
            Self::Widget { widget, .. } => Some(widget.dispatch_click(x, y, cx)),
            _ => None,
        })
    }

    /// Dispatch a scroll event to a widget.
    pub fn dispatch_scroll_event(
        &self,
        target_id: &str,
        direction: ScrollDirection,
        amount: u16,
        cx: &AppContext,
    ) -> Option<EventResult> {
        self.dispatch_event(target_id, |node| match node {
            Self::Widget { widget, .. } => Some(widget.dispatch_scroll(direction, amount, cx)),
            _ => None,
        })
    }

    /// Dispatch a drag event to a widget.
    pub fn dispatch_drag_event(
        &self,
        target_id: &str,
        x: u16,
        y: u16,
        modifiers: Modifiers,
        cx: &AppContext,
    ) -> Option<EventResult> {
        self.dispatch_event(target_id, |node| match node {
            Self::Widget { widget, .. } => Some(widget.dispatch_drag(x, y, modifiers, cx)),
            _ => None,
        })
    }

    /// Dispatch a drag release event to a widget.
    pub fn dispatch_release_event(&self, target_id: &str, cx: &AppContext) -> Option<EventResult> {
        self.dispatch_event(target_id, |node| match node {
            Self::Widget { widget, .. } => Some(widget.dispatch_release(cx)),
            _ => None,
        })
    }

    /// Dispatch a key event to a widget.
    pub fn dispatch_key_event(
        &self,
        target_id: &str,
        key: &KeyCombo,
        cx: &AppContext,
    ) -> Option<EventResult> {
        self.dispatch_event(target_id, |node| match node {
            Self::Widget { widget, .. } => Some(widget.dispatch_key(key, cx)),
            _ => None,
        })
    }

    /// Dispatch a hover event to a widget.
    pub fn dispatch_hover_event(
        &self,
        target_id: &str,
        x: u16,
        y: u16,
        cx: &AppContext,
    ) -> Option<EventResult> {
        self.dispatch_event(target_id, |node| match node {
            Self::Widget { widget, .. } => Some(widget.dispatch_hover(x, y, cx)),
            _ => None,
        })
    }

    /// Dispatch a blur event to a widget (called when focus moves away).
    ///
    /// This allows widgets to clean up state when they lose focus, such as
    /// closing open overlays (dropdowns, menus, etc.).
    pub fn dispatch_blur(&self, target_id: &str, cx: &AppContext) {
        // Use a simpler visitor pattern since blur doesn't need a return value
        fn visit_blur(node: &Node, target_id: &str, cx: &AppContext) -> bool {
            match node {
                Node::Widget { widget, .. } => {
                    if widget.id() == target_id {
                        widget.dispatch_blur(cx);
                        return true;
                    }
                    false
                }
                Node::Column { children, .. }
                | Node::Row { children, .. }
                | Node::Stack { children, .. } => {
                    children.iter().any(|c| visit_blur(c, target_id, cx))
                }
                _ => false,
            }
        }
        visit_blur(self, target_id, cx);
    }

    /// Dispatch an overlay click event to the widget that owns the overlay.
    ///
    /// This is called when a click occurs inside an overlay's area.
    /// The coordinates are relative to the overlay's top-left corner.
    pub fn dispatch_overlay_click(
        &self,
        owner_id: &str,
        x: u16,
        y: u16,
        cx: &AppContext,
    ) -> Option<EventResult> {
        self.dispatch_event(owner_id, |node| match node {
            Self::Widget { widget, .. } => Some(widget.dispatch_overlay_click(x, y, cx)),
            _ => None,
        })
    }

    /// Dispatch an overlay scroll event to the widget that owns the overlay.
    ///
    /// This is called when a scroll occurs inside an overlay's area.
    pub fn dispatch_overlay_scroll(
        &self,
        owner_id: &str,
        direction: ScrollDirection,
        amount: u16,
        cx: &AppContext,
    ) -> Option<EventResult> {
        self.dispatch_event(owner_id, |node| match node {
            Self::Widget { widget, .. } => Some(widget.dispatch_overlay_scroll(direction, amount, cx)),
            _ => None,
        })
    }

    /// Dispatch an overlay hover event to the widget that owns the overlay.
    ///
    /// This is called when the mouse hovers inside an overlay's area.
    /// The coordinates are relative to the overlay's top-left corner.
    pub fn dispatch_overlay_hover(
        &self,
        owner_id: &str,
        x: u16,
        y: u16,
        cx: &AppContext,
    ) -> Option<EventResult> {
        self.dispatch_event(owner_id, |node| match node {
            Self::Widget { widget, .. } => Some(widget.dispatch_overlay_hover(x, y, cx)),
            _ => None,
        })
    }

    // =========================================================================
    // Intrinsic Size Calculations
    // =========================================================================

    /// Calculate intrinsic width of this node
    pub fn intrinsic_width(&self) -> u16 {
        match self {
            Self::Empty => 0,
            Self::Text { content, .. } => {
                content.lines().map(|l| l.len()).max().unwrap_or(0) as u16
            }
            Self::Column { children, layout, .. } => {
                let (chrome_h, _) = layout.chrome_size();
                let max_child = children
                    .iter()
                    .map(|c| c.intrinsic_width())
                    .max()
                    .unwrap_or(0);
                max_child + chrome_h
            }
            Self::Row { children, layout, .. } => {
                let (chrome_h, _) = layout.chrome_size();
                let child_sum: u16 = children.iter().map(|c| c.intrinsic_width()).sum();
                let gaps = if children.len() > 1 {
                    layout.gap * (children.len() as u16 - 1)
                } else {
                    0
                };
                child_sum + gaps + chrome_h
            }
            Self::Stack { children, layout, .. } => {
                let (chrome_h, _) = layout.chrome_size();
                let max_child = children
                    .iter()
                    .map(|c| c.intrinsic_width())
                    .max()
                    .unwrap_or(0);
                max_child + chrome_h
            }
            Self::Widget {
                layout,
                widget,
                children,
                ..
            } => {
                let (chrome_h, _) = layout.chrome_size();
                // Get widget's intrinsic width
                let widget_width = widget.intrinsic_width();
                // For container widgets, also consider children
                let child_width = children
                    .iter()
                    .map(|c| c.intrinsic_width())
                    .max()
                    .unwrap_or(0);
                let intrinsic = widget_width.max(child_width);
                if intrinsic > 0 {
                    intrinsic + chrome_h
                } else {
                    40 + chrome_h // Default width for widgets with no intrinsic size
                }
            }
        }
    }

    /// Calculate intrinsic height of this node
    pub fn intrinsic_height(&self) -> u16 {
        match self {
            Self::Empty => 0,
            Self::Text { content, .. } => content.lines().count().max(1) as u16,
            Self::Column { children, layout, .. } => {
                let (_, chrome_v) = layout.chrome_size();
                let child_sum: u16 = children.iter().map(|c| c.intrinsic_height()).sum();
                let gaps = if children.len() > 1 {
                    layout.gap * (children.len() as u16 - 1)
                } else {
                    0
                };
                child_sum + gaps + chrome_v
            }
            Self::Row { children, layout, .. } => {
                let (_, chrome_v) = layout.chrome_size();
                let max_child = children
                    .iter()
                    .map(|c| c.intrinsic_height())
                    .max()
                    .unwrap_or(0);
                max_child + chrome_v
            }
            Self::Stack { children, layout, .. } => {
                let (_, chrome_v) = layout.chrome_size();
                let max_child = children
                    .iter()
                    .map(|c| c.intrinsic_height())
                    .max()
                    .unwrap_or(0);
                max_child + chrome_v
            }
            Self::Widget {
                layout,
                widget,
                children,
                ..
            } => {
                let (_, chrome_v) = layout.chrome_size();
                // Get widget's intrinsic height
                let widget_height = widget.intrinsic_height();
                // For container widgets, also consider children (sum for vertical stacking)
                let child_height: u16 = children.iter().map(|c| c.intrinsic_height()).sum();
                let intrinsic = widget_height.max(child_height);
                intrinsic + chrome_v
            }
        }
    }
}
