//! Node types for the page tree.

mod layout;

pub use layout::{Align, Border, Direction, Justify, Layout, Size};

use crate::widgets::events::{EventResult, WidgetEvents};
use crate::widgets::list::AnyList;
use crate::widgets::table::AnyTable;
use crate::widgets::tree::AnyTree;
use crate::widgets::{
    AnySelectable, AnyWidget, Checkbox, Input, RadioGroup, ScrollArea, WidgetHandlers,
};
use crate::context::AppContext;
use crate::events::{Modifiers, ScrollDirection};
use crate::keybinds::{HandlerId, KeyCombo};
use crate::style::Style;

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
    Widget {
        /// The widget instance (type-erased)
        widget: Box<dyn AnyWidget>,
        /// Event handlers for this widget
        handlers: WidgetHandlers,
        /// Style
        style: Style,
        /// Layout properties
        layout: Layout,
    },

    // =========================================================================
    // Legacy widget variants (to be removed in Phase 5)
    // These exist temporarily while we migrate built-in widgets to AnyWidget
    // =========================================================================

    /// Text input field (legacy - will be migrated to Widget)
    #[deprecated(note = "Use Widget variant with AnyWidget instead")]
    Input {
        value: String,
        placeholder: String,
        on_change: Option<HandlerId>,
        on_submit: Option<HandlerId>,
        id: String,
        style: Style,
        widget: Option<Input>,
    },

    /// Clickable button (legacy - will be migrated to Widget)
    #[deprecated(note = "Use Widget variant with AnyWidget instead")]
    Button {
        label: String,
        on_click: Option<HandlerId>,
        id: String,
        style: Style,
    },

    /// Checkbox toggle (legacy - will be migrated to Widget)
    #[deprecated(note = "Use Widget variant with AnyWidget instead")]
    Checkbox {
        id: String,
        style: Style,
        widget: Checkbox,
        on_change: Option<HandlerId>,
    },

    /// Radio group (legacy - will be migrated to Widget)
    #[deprecated(note = "Use Widget variant with AnyWidget instead")]
    RadioGroup {
        id: String,
        style: Style,
        widget: RadioGroup,
        on_change: Option<HandlerId>,
    },

    /// ScrollArea container (legacy - will be migrated to Widget)
    #[deprecated(note = "Use Widget variant with AnyWidget instead")]
    ScrollArea {
        child: Box<Node>,
        id: String,
        style: Style,
        layout: Layout,
        widget: ScrollArea,
    },

    /// Virtualized list (legacy - will be migrated to Widget)
    #[deprecated(note = "Use Widget variant with AnyWidget instead")]
    List {
        id: String,
        style: Style,
        layout: Layout,
        widget: Box<dyn AnyList>,
        on_activate: Option<HandlerId>,
        on_selection_change: Option<HandlerId>,
        on_cursor_move: Option<HandlerId>,
        on_scroll: Option<HandlerId>,
    },

    /// Virtualized tree (legacy - will be migrated to Widget)
    #[deprecated(note = "Use Widget variant with AnyWidget instead")]
    Tree {
        id: String,
        style: Style,
        layout: Layout,
        widget: Box<dyn AnyTree>,
        on_activate: Option<HandlerId>,
        on_expand: Option<HandlerId>,
        on_collapse: Option<HandlerId>,
        on_selection_change: Option<HandlerId>,
        on_cursor_move: Option<HandlerId>,
    },

    /// Virtualized table (legacy - will be migrated to Widget)
    #[deprecated(note = "Use Widget variant with AnyWidget instead")]
    Table {
        id: String,
        style: Style,
        layout: Layout,
        widget: Box<dyn AnyTable>,
        on_activate: Option<HandlerId>,
        on_selection_change: Option<HandlerId>,
        on_cursor_move: Option<HandlerId>,
        on_sort: Option<HandlerId>,
    },
}

// Allow deprecated variants within this module during migration
#[allow(deprecated)]
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

    /// Create a button node (legacy)
    #[deprecated(note = "Use Widget variant instead")]
    pub fn button(label: impl Into<String>) -> Self {
        Self::Button {
            label: label.into(),
            on_click: None,
            id: String::new(),
            style: Style::new(),
        }
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
        }
    }

    // =========================================================================
    // Node Properties
    // =========================================================================

    /// Check if this node is focusable
    pub fn is_focusable(&self) -> bool {
        match self {
            Self::Widget { widget, .. } => widget.is_focusable(),
            // Legacy variants
            Self::Input { .. }
            | Self::Button { .. }
            | Self::Checkbox { .. }
            | Self::RadioGroup { .. }
            | Self::ScrollArea { .. }
            | Self::List { .. }
            | Self::Tree { .. }
            | Self::Table { .. } => true,
            _ => false,
        }
    }

    /// Get the element ID if any
    pub fn id(&self) -> Option<&str> {
        match self {
            Self::Widget { widget, .. } => Some(widget.id().leak()), // TODO: avoid leak
            // Legacy variants
            Self::Input { id, .. }
            | Self::Button { id, .. }
            | Self::Checkbox { id, .. }
            | Self::RadioGroup { id, .. }
            | Self::ScrollArea { id, .. }
            | Self::List { id, .. }
            | Self::Tree { id, .. }
            | Self::Table { id, .. } => {
                if id.is_empty() {
                    None
                } else {
                    Some(id.as_str())
                }
            }
            _ => None,
        }
    }

    /// Check if this node captures text input when focused
    pub fn captures_input(&self) -> bool {
        match self {
            Self::Widget { widget, .. } => widget.captures_input(),
            Self::Input { .. } => true,
            _ => false,
        }
    }

    // =========================================================================
    // Focus Management
    // =========================================================================

    /// Collect all focusable element IDs from this node and its children (in tree order)
    pub fn collect_focusable_ids(&self, ids: &mut Vec<String>) {
        match self {
            Self::Widget { widget, .. } => {
                if widget.is_focusable() {
                    ids.push(widget.id());
                }
            }
            // Legacy variants
            Self::Input { id, .. }
            | Self::Button { id, .. }
            | Self::Checkbox { id, .. }
            | Self::RadioGroup { id, .. }
                if !id.is_empty() =>
            {
                ids.push(id.clone());
            }
            Self::ScrollArea { id, child, .. } => {
                if !id.is_empty() {
                    ids.push(id.clone());
                }
                child.collect_focusable_ids(ids);
            }
            Self::List { id, .. } | Self::Tree { id, .. } | Self::Table { id, .. } => {
                if !id.is_empty() {
                    ids.push(id.clone());
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
            Self::Widget { widget, .. } if widget.id() == target_id => widget.captures_input(),
            Self::Input { id, .. } if id == target_id => true,
            Self::Column { children, .. }
            | Self::Row { children, .. }
            | Self::Stack { children, .. } => {
                children.iter().any(|c| c.element_captures_input(target_id))
            }
            Self::ScrollArea { child, .. } => child.element_captures_input(target_id),
            _ => false,
        }
    }

    // =========================================================================
    // Widget Access (New Unified API)
    // =========================================================================

    /// Get a widget by ID.
    ///
    /// This is the unified way to access any widget implementing `AnyWidget`.
    pub fn get_widget(&self, target_id: &str) -> Option<&dyn AnyWidget> {
        match self {
            Self::Widget { widget, .. } if widget.id() == target_id => Some(widget.as_ref()),
            Self::Column { children, .. }
            | Self::Row { children, .. }
            | Self::Stack { children, .. } => {
                children.iter().find_map(|c| c.get_widget(target_id))
            }
            Self::ScrollArea { child, .. } => child.get_widget(target_id),
            _ => None,
        }
    }

    /// Get widget handlers by ID.
    pub fn get_widget_handlers(&self, target_id: &str) -> Option<&WidgetHandlers> {
        match self {
            Self::Widget { widget, handlers, .. } if widget.id() == target_id => Some(handlers),
            Self::Column { children, .. }
            | Self::Row { children, .. }
            | Self::Stack { children, .. } => {
                children.iter().find_map(|c| c.get_widget_handlers(target_id))
            }
            Self::ScrollArea { child, .. } => child.get_widget_handlers(target_id),
            _ => None,
        }
    }

    // =========================================================================
    // Legacy Widget Access (to be removed in Phase 5)
    // =========================================================================

    /// Get the current value of an input element by ID
    pub fn input_value(&self, target_id: &str) -> Option<String> {
        match self {
            Self::Input { id, value, .. } if id == target_id => Some(value.clone()),
            Self::Column { children, .. }
            | Self::Row { children, .. }
            | Self::Stack { children, .. } => {
                children.iter().find_map(|c| c.input_value(target_id))
            }
            Self::ScrollArea { child, .. } => child.input_value(target_id),
            _ => None,
        }
    }

    /// Get the Input widget for an input element by ID
    pub fn get_input_component(&self, target_id: &str) -> Option<&Input> {
        match self {
            Self::Input { id, widget, .. } if id == target_id => widget.as_ref(),
            Self::Column { children, .. }
            | Self::Row { children, .. }
            | Self::Stack { children, .. } => children
                .iter()
                .find_map(|c| c.get_input_component(target_id)),
            Self::ScrollArea { child, .. } => child.get_input_component(target_id),
            _ => None,
        }
    }

    /// Get the Checkbox widget for a checkbox element by ID
    pub fn get_checkbox_component(&self, target_id: &str) -> Option<&Checkbox> {
        match self {
            Self::Checkbox { id, widget, .. } if id == target_id => Some(widget),
            Self::Column { children, .. }
            | Self::Row { children, .. }
            | Self::Stack { children, .. } => children
                .iter()
                .find_map(|c| c.get_checkbox_component(target_id)),
            Self::ScrollArea { child, .. } => child.get_checkbox_component(target_id),
            _ => None,
        }
    }

    /// Get the RadioGroup widget for a radio group element by ID
    pub fn get_radio_group_component(&self, target_id: &str) -> Option<&RadioGroup> {
        match self {
            Self::RadioGroup { id, widget, .. } if id == target_id => Some(widget),
            Self::Column { children, .. }
            | Self::Row { children, .. }
            | Self::Stack { children, .. } => children
                .iter()
                .find_map(|c| c.get_radio_group_component(target_id)),
            Self::ScrollArea { child, .. } => child.get_radio_group_component(target_id),
            _ => None,
        }
    }

    /// Get the ScrollArea widget for a scroll area element by ID
    pub fn get_scroll_area_component(&self, target_id: &str) -> Option<&ScrollArea> {
        match self {
            Self::ScrollArea { id, widget, .. } if id == target_id => Some(widget),
            Self::Column { children, .. }
            | Self::Row { children, .. }
            | Self::Stack { children, .. } => children
                .iter()
                .find_map(|c| c.get_scroll_area_component(target_id)),
            Self::ScrollArea { child, .. } => child.get_scroll_area_component(target_id),
            _ => None,
        }
    }

    /// Get the List widget for a list element by ID
    pub fn get_list_component(&self, target_id: &str) -> Option<&dyn AnyList> {
        match self {
            Self::List { id, widget, .. } if id == target_id => Some(widget.as_ref()),
            Self::Column { children, .. }
            | Self::Row { children, .. }
            | Self::Stack { children, .. } => children
                .iter()
                .find_map(|c| c.get_list_component(target_id)),
            Self::ScrollArea { child, .. } => child.get_list_component(target_id),
            _ => None,
        }
    }

    /// Get the Tree widget for a tree element by ID
    pub fn get_tree_component(&self, target_id: &str) -> Option<&dyn AnyTree> {
        match self {
            Self::Tree { id, widget, .. } if id == target_id => Some(widget.as_ref()),
            Self::Column { children, .. }
            | Self::Row { children, .. }
            | Self::Stack { children, .. } => children
                .iter()
                .find_map(|c| c.get_tree_component(target_id)),
            Self::ScrollArea { child, .. } => child.get_tree_component(target_id),
            _ => None,
        }
    }

    /// Get the Table widget for a table element by ID
    pub fn get_table_component(&self, target_id: &str) -> Option<&dyn AnyTable> {
        match self {
            Self::Table { id, widget, .. } if id == target_id => Some(widget.as_ref()),
            Self::Column { children, .. }
            | Self::Row { children, .. }
            | Self::Stack { children, .. } => children
                .iter()
                .find_map(|c| c.get_table_component(target_id)),
            Self::ScrollArea { child, .. } => child.get_table_component(target_id),
            _ => None,
        }
    }

    /// Get a type-erased selectable widget (List, Tree, or Table) by ID.
    pub fn get_selectable_component(&self, target_id: &str) -> Option<&dyn AnySelectable> {
        if let Some(list) = self.get_list_component(target_id) {
            return Some(list.as_any_selectable());
        }
        if let Some(tree) = self.get_tree_component(target_id) {
            return Some(tree.as_any_selectable());
        }
        if let Some(table) = self.get_table_component(target_id) {
            return Some(table.as_any_selectable());
        }
        None
    }

    // =========================================================================
    // Handler Getters
    // =========================================================================

    /// Get the handler for a focusable element (on_click for buttons, on_submit for inputs, on_activate for lists)
    pub fn get_submit_handler(&self, target_id: &str) -> Option<HandlerId> {
        match self {
            // New Widget variant
            Self::Widget { widget, handlers, .. } if widget.id() == target_id => {
                handlers.on_activate.clone().or_else(|| handlers.on_submit.clone())
            }
            // Legacy variants
            Self::Button { id, on_click, .. } if id == target_id => on_click.clone(),
            Self::Input { id, on_submit, .. } if id == target_id => on_submit.clone(),
            Self::List { id, on_activate, .. } if id == target_id => on_activate.clone(),
            Self::Tree { id, on_activate, .. } if id == target_id => on_activate.clone(),
            Self::Table { id, on_activate, .. } if id == target_id => on_activate.clone(),
            Self::Column { children, .. }
            | Self::Row { children, .. }
            | Self::Stack { children, .. } => children
                .iter()
                .find_map(|c| c.get_submit_handler(target_id)),
            Self::ScrollArea { child, .. } => child.get_submit_handler(target_id),
            _ => None,
        }
    }

    /// Get the on_change handler for an input or checkbox element by ID
    pub fn get_change_handler(&self, target_id: &str) -> Option<HandlerId> {
        match self {
            Self::Widget { widget, handlers, .. } if widget.id() == target_id => {
                handlers.on_change.clone()
            }
            Self::Input { id, on_change, .. } if id == target_id => on_change.clone(),
            Self::Checkbox { id, on_change, .. } if id == target_id => on_change.clone(),
            Self::RadioGroup { id, on_change, .. } if id == target_id => on_change.clone(),
            Self::Column { children, .. }
            | Self::Row { children, .. }
            | Self::Stack { children, .. } => children
                .iter()
                .find_map(|c| c.get_change_handler(target_id)),
            Self::ScrollArea { child, .. } => child.get_change_handler(target_id),
            _ => None,
        }
    }

    /// Get the on_cursor_move handler by ID.
    pub fn get_cursor_handler(&self, target_id: &str) -> Option<HandlerId> {
        match self {
            Self::Widget { widget, handlers, .. } if widget.id() == target_id => {
                handlers.on_cursor_move.clone()
            }
            Self::List { id, on_cursor_move, .. } if id == target_id => on_cursor_move.clone(),
            Self::Tree { id, on_cursor_move, .. } if id == target_id => on_cursor_move.clone(),
            Self::Table { id, on_cursor_move, .. } if id == target_id => on_cursor_move.clone(),
            Self::Column { children, .. }
            | Self::Row { children, .. }
            | Self::Stack { children, .. } => children
                .iter()
                .find_map(|c| c.get_cursor_handler(target_id)),
            Self::ScrollArea { child, .. } => child.get_cursor_handler(target_id),
            _ => None,
        }
    }

    /// Get the on_selection_change handler by ID.
    pub fn get_selection_handler(&self, target_id: &str) -> Option<HandlerId> {
        match self {
            Self::Widget { widget, handlers, .. } if widget.id() == target_id => {
                handlers.on_selection_change.clone()
            }
            Self::List { id, on_selection_change, .. } if id == target_id => on_selection_change.clone(),
            Self::Tree { id, on_selection_change, .. } if id == target_id => on_selection_change.clone(),
            Self::Table { id, on_selection_change, .. } if id == target_id => on_selection_change.clone(),
            Self::Column { children, .. }
            | Self::Row { children, .. }
            | Self::Stack { children, .. } => children
                .iter()
                .find_map(|c| c.get_selection_handler(target_id)),
            Self::ScrollArea { child, .. } => child.get_selection_handler(target_id),
            _ => None,
        }
    }

    /// Get the on_expand handler by ID.
    pub fn get_expand_handler(&self, target_id: &str) -> Option<HandlerId> {
        match self {
            Self::Widget { widget, handlers, .. } if widget.id() == target_id => {
                handlers.on_expand.clone()
            }
            Self::Tree { id, on_expand, .. } if id == target_id => on_expand.clone(),
            Self::Column { children, .. }
            | Self::Row { children, .. }
            | Self::Stack { children, .. } => children
                .iter()
                .find_map(|c| c.get_expand_handler(target_id)),
            Self::ScrollArea { child, .. } => child.get_expand_handler(target_id),
            _ => None,
        }
    }

    /// Get the on_collapse handler by ID.
    pub fn get_collapse_handler(&self, target_id: &str) -> Option<HandlerId> {
        match self {
            Self::Widget { widget, handlers, .. } if widget.id() == target_id => {
                handlers.on_collapse.clone()
            }
            Self::Tree { id, on_collapse, .. } if id == target_id => on_collapse.clone(),
            Self::Column { children, .. }
            | Self::Row { children, .. }
            | Self::Stack { children, .. } => children
                .iter()
                .find_map(|c| c.get_collapse_handler(target_id)),
            Self::ScrollArea { child, .. } => child.get_collapse_handler(target_id),
            _ => None,
        }
    }

    /// Get the on_sort handler by ID.
    pub fn get_sort_handler(&self, target_id: &str) -> Option<HandlerId> {
        match self {
            Self::Widget { widget, handlers, .. } if widget.id() == target_id => {
                handlers.on_sort.clone()
            }
            Self::Table { id, on_sort, .. } if id == target_id => on_sort.clone(),
            Self::Column { children, .. }
            | Self::Row { children, .. }
            | Self::Stack { children, .. } => {
                children.iter().find_map(|c| c.get_sort_handler(target_id))
            }
            Self::ScrollArea { child, .. } => child.get_sort_handler(target_id),
            _ => None,
        }
    }

    /// Get the on_scroll handler by ID
    pub fn get_list_scroll_handler(&self, target_id: &str) -> Option<HandlerId> {
        match self {
            Self::Widget { widget, handlers, .. } if widget.id() == target_id => {
                handlers.on_scroll.clone()
            }
            Self::List { id, on_scroll, .. } if id == target_id => on_scroll.clone(),
            Self::Column { children, .. }
            | Self::Row { children, .. }
            | Self::Stack { children, .. } => children
                .iter()
                .find_map(|c| c.get_list_scroll_handler(target_id)),
            Self::ScrollArea { child, .. } => child.get_list_scroll_handler(target_id),
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
            Self::Column { children, .. }
            | Self::Row { children, .. }
            | Self::Stack { children, .. } => children
                .iter()
                .find_map(|c| c.dispatch_event(target_id, visitor)),
            Self::ScrollArea { child, .. } => child.dispatch_event(target_id, visitor),
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
            // Legacy variants
            Self::ScrollArea { widget, .. } => Some(widget.on_click(x, y, cx)),
            Self::Input { widget: Some(widget), .. } => Some(widget.on_click(x, y, cx)),
            Self::Input { widget: None, .. } => Some(EventResult::Ignored),
            Self::List { widget, .. } => Some(widget.on_click(x, y, cx)),
            Self::Tree { widget, .. } => Some(widget.on_click(x, y, cx)),
            Self::Table { widget, .. } => Some(widget.on_click(x, y, cx)),
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
            Self::ScrollArea { widget, .. } => Some(widget.on_scroll(direction, amount, cx)),
            Self::List { widget, .. } => Some(widget.on_scroll(direction, amount, cx)),
            Self::Tree { widget, .. } => Some(widget.on_scroll(direction, amount, cx)),
            Self::Table { widget, .. } => Some(widget.on_scroll(direction, amount, cx)),
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
            Self::ScrollArea { widget, .. } => Some(widget.on_drag(x, y, modifiers, cx)),
            Self::List { widget, .. } => Some(widget.on_drag(x, y, modifiers, cx)),
            Self::Tree { widget, .. } => Some(widget.on_drag(x, y, modifiers, cx)),
            Self::Table { widget, .. } => Some(widget.on_drag(x, y, modifiers, cx)),
            _ => None,
        })
    }

    /// Dispatch a drag release event to a widget.
    pub fn dispatch_release_event(&self, target_id: &str, cx: &AppContext) -> Option<EventResult> {
        self.dispatch_event(target_id, |node| match node {
            Self::Widget { widget, .. } => Some(widget.dispatch_release(cx)),
            Self::ScrollArea { widget, .. } => Some(widget.on_release(cx)),
            Self::List { widget, .. } => Some(widget.on_release(cx)),
            Self::Tree { widget, .. } => Some(widget.on_release(cx)),
            Self::Table { widget, .. } => Some(widget.on_release(cx)),
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
            Self::Input { widget: Some(widget), .. } => Some(widget.on_key(key, cx)),
            Self::Input { widget: None, .. } => Some(EventResult::Ignored),
            Self::Checkbox { widget, .. } => Some(widget.on_key(key, cx)),
            Self::RadioGroup { widget, .. } => Some(widget.on_key(key, cx)),
            Self::ScrollArea { widget, .. } => Some(widget.on_key(key, cx)),
            Self::List { widget, .. } => Some(widget.on_key(key, cx)),
            Self::Tree { widget, .. } => Some(widget.on_key(key, cx)),
            Self::Table { widget, .. } => Some(widget.on_key(key, cx)),
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
            Self::List { widget, .. } => Some(widget.on_hover(x, y, cx)),
            Self::Tree { widget, .. } => Some(widget.on_hover(x, y, cx)),
            Self::Table { widget, .. } => Some(widget.on_hover(x, y, cx)),
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
            Self::Widget { layout, .. } => {
                let (chrome_h, _) = layout.chrome_size();
                40 + chrome_h // Default width for widgets
            }
            // Legacy variants
            Self::Input { value, placeholder, .. } => {
                let content_len = if value.is_empty() {
                    placeholder.len()
                } else {
                    value.len()
                };
                (content_len + 5).max(15) as u16
            }
            Self::Button { label, .. } => (label.len() + 4) as u16,
            Self::Checkbox { widget, .. } => {
                let label = widget.label();
                if label.is_empty() { 1 } else { (label.len() + 2) as u16 }
            }
            Self::RadioGroup { widget, .. } => {
                widget.options().iter().map(|l| l.len() + 2).max().unwrap_or(1) as u16
            }
            Self::ScrollArea { child, layout, .. } => {
                let (chrome_h, _) = layout.chrome_size();
                child.intrinsic_width() + chrome_h
            }
            Self::List { layout, .. } | Self::Tree { layout, .. } => {
                let (chrome_h, _) = layout.chrome_size();
                40 + chrome_h
            }
            Self::Table { layout, widget, .. } => {
                let (chrome_h, _) = layout.chrome_size();
                widget.total_width() + chrome_h
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
            Self::Widget { layout, .. } => {
                let (_, chrome_v) = layout.chrome_size();
                1 + chrome_v // Default height for widgets
            }
            // Legacy variants
            Self::Input { .. } | Self::Button { .. } | Self::Checkbox { .. } => 1,
            Self::RadioGroup { widget, .. } => widget.len().max(1) as u16,
            Self::ScrollArea { child, layout, .. } => {
                let (_, chrome_v) = layout.chrome_size();
                child.intrinsic_height() + chrome_v
            }
            Self::List { layout, widget, .. } => {
                let (_, chrome_v) = layout.chrome_size();
                widget.total_height() + chrome_v
            }
            Self::Tree { layout, widget, .. } => {
                let (_, chrome_v) = layout.chrome_size();
                widget.total_height() + chrome_v
            }
            Self::Table { layout, widget, .. } => {
                let (_, chrome_v) = layout.chrome_size();
                widget.total_height() + 1 + chrome_v
            }
        }
    }
}

