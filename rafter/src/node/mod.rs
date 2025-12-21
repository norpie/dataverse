//! Node types for the view tree.

mod layout;

pub use layout::{Align, Border, Direction, Justify, Layout, Size};

use crate::components::events::{ComponentEvents, EventResult};
use crate::components::list::AnyList;
use crate::components::table::AnyTable;
use crate::components::tree::AnyTree;
use crate::components::{Input, ScrollArea};
use crate::context::AppContext;
use crate::events::{Modifiers, ScrollDirection};
use crate::keybinds::{HandlerId, KeyCombo};
use crate::style::Style;

/// A node in the view tree
#[derive(Debug, Clone, Default)]
pub enum Node {
    /// Empty node (renders nothing)
    #[default]
    Empty,

    /// Text content
    Text { content: String, style: Style },

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

    /// Text input field
    Input {
        /// Current input value
        value: String,
        /// Placeholder text
        placeholder: String,
        /// Handler for value changes
        on_change: Option<HandlerId>,
        /// Handler for submit (Enter)
        on_submit: Option<HandlerId>,
        /// Element ID for focus (auto-generated if not specified)
        id: String,
        /// Style
        style: Style,
        /// Bound Input component (if using bind: syntax)
        component: Option<Input>,
    },

    /// Clickable button
    Button {
        /// Button label
        label: String,
        /// Click handler
        on_click: Option<HandlerId>,
        /// Element ID for focus (auto-generated if not specified)
        id: String,
        /// Style
        style: Style,
    },

    /// ScrollArea container
    ScrollArea {
        /// Child node (content to scroll)
        child: Box<Node>,
        /// Element ID
        id: String,
        /// Style
        style: Style,
        /// Layout properties
        layout: Layout,
        /// Bound ScrollArea component
        component: ScrollArea,
    },

    /// Virtualized list
    List {
        /// Element ID
        id: String,
        /// Style
        style: Style,
        /// Layout properties
        layout: Layout,
        /// The list component (type-erased)
        component: Box<dyn AnyList>,
        /// Handler for item activation
        on_activate: Option<HandlerId>,
        /// Handler for selection changes
        on_selection_change: Option<HandlerId>,
        /// Handler for cursor movement
        on_cursor_move: Option<HandlerId>,
        /// Handler for scroll events (useful for pagination / infinite scroll)
        on_scroll: Option<HandlerId>,
    },

    /// Virtualized tree
    Tree {
        /// Element ID
        id: String,
        /// Style
        style: Style,
        /// Layout properties
        layout: Layout,
        /// The tree component (type-erased)
        component: Box<dyn AnyTree>,
        /// Handler for node activation
        on_activate: Option<HandlerId>,
        /// Handler for node expansion
        on_expand: Option<HandlerId>,
        /// Handler for node collapse
        on_collapse: Option<HandlerId>,
        /// Handler for selection changes
        on_selection_change: Option<HandlerId>,
        /// Handler for cursor movement
        on_cursor_move: Option<HandlerId>,
    },

    /// Virtualized table with columns
    Table {
        /// Element ID
        id: String,
        /// Style
        style: Style,
        /// Layout properties
        layout: Layout,
        /// The table component (type-erased)
        component: Box<dyn AnyTable>,
        /// Handler for row activation
        on_activate: Option<HandlerId>,
        /// Handler for selection changes
        on_selection_change: Option<HandlerId>,
        /// Handler for cursor movement
        on_cursor_move: Option<HandlerId>,
        /// Handler for column sort
        on_sort: Option<HandlerId>,
    },
}

impl Node {
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

    /// Create a button node
    pub fn button(label: impl Into<String>) -> Self {
        Self::Button {
            label: label.into(),
            on_click: None,
            id: String::new(),
            style: Style::new(),
        }
    }

    /// Check if this node is focusable
    pub fn is_focusable(&self) -> bool {
        matches!(
            self,
            Self::Input { .. }
                | Self::Button { .. }
                | Self::ScrollArea { .. }
                | Self::List { .. }
                | Self::Tree { .. }
                | Self::Table { .. }
        )
    }

    /// Get the element ID if any
    pub fn id(&self) -> Option<&str> {
        match self {
            Self::Input { id, .. }
            | Self::Button { id, .. }
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
        matches!(self, Self::Input { .. })
    }

    /// Collect all focusable element IDs from this node and its children (in tree order)
    pub fn collect_focusable_ids(&self, ids: &mut Vec<String>) {
        match self {
            Self::Input { id, .. } | Self::Button { id, .. } if !id.is_empty() => {
                ids.push(id.clone());
            }
            Self::ScrollArea { id, child, .. } => {
                // ScrollArea itself is focusable
                if !id.is_empty() {
                    ids.push(id.clone());
                }
                // Also collect focusable children inside the scroll area
                child.collect_focusable_ids(ids);
            }
            Self::List { id, .. } | Self::Tree { id, .. } | Self::Table { id, .. } => {
                // List/Tree/Table is focusable (no children to collect)
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

    /// Get the handler for a focusable element (on_click for buttons, on_submit for inputs)
    pub fn get_submit_handler(&self, target_id: &str) -> Option<HandlerId> {
        match self {
            Self::Button { id, on_click, .. } if id == target_id => on_click.clone(),
            Self::Input { id, on_submit, .. } if id == target_id => on_submit.clone(),
            Self::List {
                id, on_activate, ..
            } if id == target_id => on_activate.clone(),
            Self::Tree {
                id, on_activate, ..
            } if id == target_id => on_activate.clone(),
            Self::Table {
                id, on_activate, ..
            } if id == target_id => on_activate.clone(),
            Self::Column { children, .. }
            | Self::Row { children, .. }
            | Self::Stack { children, .. } => children
                .iter()
                .find_map(|c| c.get_submit_handler(target_id)),
            Self::ScrollArea { child, .. } => child.get_submit_handler(target_id),
            _ => None,
        }
    }

    /// Get the on_change handler for an input element by ID
    pub fn get_change_handler(&self, target_id: &str) -> Option<HandlerId> {
        match self {
            Self::Input { id, on_change, .. } if id == target_id => on_change.clone(),
            Self::Column { children, .. }
            | Self::Row { children, .. }
            | Self::Stack { children, .. } => children
                .iter()
                .find_map(|c| c.get_change_handler(target_id)),
            Self::ScrollArea { child, .. } => child.get_change_handler(target_id),
            _ => None,
        }
    }

    /// Get the Input component for an input element by ID
    pub fn get_input_component(&self, target_id: &str) -> Option<&Input> {
        match self {
            Self::Input { id, component, .. } if id == target_id => component.as_ref(),
            Self::Column { children, .. }
            | Self::Row { children, .. }
            | Self::Stack { children, .. } => children
                .iter()
                .find_map(|c| c.get_input_component(target_id)),
            Self::ScrollArea { child, .. } => child.get_input_component(target_id),
            _ => None,
        }
    }

    /// Get the ScrollArea component for a scroll area element by ID
    pub fn get_scroll_area_component(&self, target_id: &str) -> Option<&ScrollArea> {
        match self {
            Self::ScrollArea { id, component, .. } if id == target_id => Some(component),
            Self::Column { children, .. }
            | Self::Row { children, .. }
            | Self::Stack { children, .. } => children
                .iter()
                .find_map(|c| c.get_scroll_area_component(target_id)),
            Self::ScrollArea { child, .. } => child.get_scroll_area_component(target_id),
            _ => None,
        }
    }

    /// Get the List component for a list element by ID
    pub fn get_list_component(&self, target_id: &str) -> Option<&dyn AnyList> {
        match self {
            Self::List { id, component, .. } if id == target_id => Some(component.as_ref()),
            Self::Column { children, .. }
            | Self::Row { children, .. }
            | Self::Stack { children, .. } => children
                .iter()
                .find_map(|c| c.get_list_component(target_id)),
            Self::ScrollArea { child, .. } => child.get_list_component(target_id),
            _ => None,
        }
    }

    // =========================================================================
    // Unified handler getters (work for list/tree/table)
    // =========================================================================

    /// Get the on_cursor_move handler for any component (list/tree/table) by ID.
    pub fn get_cursor_handler(&self, target_id: &str) -> Option<HandlerId> {
        match self {
            Self::List {
                id, on_cursor_move, ..
            } if id == target_id => on_cursor_move.clone(),
            Self::Tree {
                id, on_cursor_move, ..
            } if id == target_id => on_cursor_move.clone(),
            Self::Table {
                id, on_cursor_move, ..
            } if id == target_id => on_cursor_move.clone(),
            Self::Column { children, .. }
            | Self::Row { children, .. }
            | Self::Stack { children, .. } => {
                children.iter().find_map(|c| c.get_cursor_handler(target_id))
            }
            Self::ScrollArea { child, .. } => child.get_cursor_handler(target_id),
            _ => None,
        }
    }

    /// Get the on_selection_change handler for any component (list/tree/table) by ID.
    pub fn get_selection_handler(&self, target_id: &str) -> Option<HandlerId> {
        match self {
            Self::List {
                id,
                on_selection_change,
                ..
            } if id == target_id => on_selection_change.clone(),
            Self::Tree {
                id,
                on_selection_change,
                ..
            } if id == target_id => on_selection_change.clone(),
            Self::Table {
                id,
                on_selection_change,
                ..
            } if id == target_id => on_selection_change.clone(),
            Self::Column { children, .. }
            | Self::Row { children, .. }
            | Self::Stack { children, .. } => children
                .iter()
                .find_map(|c| c.get_selection_handler(target_id)),
            Self::ScrollArea { child, .. } => child.get_selection_handler(target_id),
            _ => None,
        }
    }

    /// Get the on_expand handler for a tree element by ID.
    pub fn get_expand_handler(&self, target_id: &str) -> Option<HandlerId> {
        match self {
            Self::Tree { id, on_expand, .. } if id == target_id => on_expand.clone(),
            Self::Column { children, .. }
            | Self::Row { children, .. }
            | Self::Stack { children, .. } => {
                children.iter().find_map(|c| c.get_expand_handler(target_id))
            }
            Self::ScrollArea { child, .. } => child.get_expand_handler(target_id),
            _ => None,
        }
    }

    /// Get the on_collapse handler for a tree element by ID.
    pub fn get_collapse_handler(&self, target_id: &str) -> Option<HandlerId> {
        match self {
            Self::Tree {
                id, on_collapse, ..
            } if id == target_id => on_collapse.clone(),
            Self::Column { children, .. }
            | Self::Row { children, .. }
            | Self::Stack { children, .. } => children
                .iter()
                .find_map(|c| c.get_collapse_handler(target_id)),
            Self::ScrollArea { child, .. } => child.get_collapse_handler(target_id),
            _ => None,
        }
    }

    /// Get the on_sort handler for a table element by ID.
    pub fn get_sort_handler(&self, target_id: &str) -> Option<HandlerId> {
        match self {
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

    /// Get the on_scroll handler for a list element by ID
    pub fn get_list_scroll_handler(&self, target_id: &str) -> Option<HandlerId> {
        match self {
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

    /// Get the Tree component for a tree element by ID
    pub fn get_tree_component(&self, target_id: &str) -> Option<&dyn AnyTree> {
        match self {
            Self::Tree { id, component, .. } if id == target_id => Some(component.as_ref()),
            Self::Column { children, .. }
            | Self::Row { children, .. }
            | Self::Stack { children, .. } => children
                .iter()
                .find_map(|c| c.get_tree_component(target_id)),
            Self::ScrollArea { child, .. } => child.get_tree_component(target_id),
            _ => None,
        }
    }

    /// Get the Table component for a table element by ID
    pub fn get_table_component(&self, target_id: &str) -> Option<&dyn AnyTable> {
        match self {
            Self::Table { id, component, .. } if id == target_id => Some(component.as_ref()),
            Self::Column { children, .. }
            | Self::Row { children, .. }
            | Self::Stack { children, .. } => children
                .iter()
                .find_map(|c| c.get_table_component(target_id)),
            Self::ScrollArea { child, .. } => child.get_table_component(target_id),
            _ => None,
        }
    }

    /// Dispatch an event to a component by ID using a visitor function.
    ///
    /// This is the core tree traversal logic used by all dispatch_*_event methods.
    /// The visitor function is called when the target node is found.
    fn dispatch_event<F>(&self, target_id: &str, visitor: F) -> Option<EventResult>
    where
        F: Fn(&Node) -> Option<EventResult> + Copy,
    {
        // First, check if this node matches (visitor will check if it's the right type)
        if let Some(id) = self.id()
            && id == target_id
            && let Some(result) = visitor(self)
        {
            return Some(result);
        }

        // Recurse into children
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

    /// Dispatch a click event to a component.
    ///
    /// Finds the component with the given ID and delegates to its `on_click` handler.
    pub fn dispatch_click_event(
        &self,
        target_id: &str,
        x: u16,
        y: u16,
        cx: &AppContext,
    ) -> Option<EventResult> {
        self.dispatch_event(target_id, |node| match node {
            Self::ScrollArea { component, .. } => Some(component.on_click(x, y, cx)),
            Self::Input {
                component: Some(component),
                ..
            } => Some(component.on_click(x, y, cx)),
            Self::Input {
                component: None, ..
            } => Some(EventResult::Ignored),
            Self::List { component, .. } => Some(component.on_click(x, y, cx)),
            Self::Tree { component, .. } => Some(component.on_click(x, y, cx)),
            Self::Table { component, .. } => Some(component.on_click(x, y, cx)),
            _ => None,
        })
    }

    /// Dispatch a scroll event to a component.
    pub fn dispatch_scroll_event(
        &self,
        target_id: &str,
        direction: ScrollDirection,
        amount: u16,
        cx: &AppContext,
    ) -> Option<EventResult> {
        self.dispatch_event(target_id, |node| match node {
            Self::ScrollArea { component, .. } => Some(component.on_scroll(direction, amount, cx)),
            Self::List { component, .. } => Some(component.on_scroll(direction, amount, cx)),
            Self::Tree { component, .. } => Some(component.on_scroll(direction, amount, cx)),
            Self::Table { component, .. } => Some(component.on_scroll(direction, amount, cx)),
            _ => None,
        })
    }

    /// Dispatch a drag event to a component.
    ///
    /// The target ID is typically stored from a previous `StartDrag` result.
    pub fn dispatch_drag_event(
        &self,
        target_id: &str,
        x: u16,
        y: u16,
        modifiers: Modifiers,
        cx: &AppContext,
    ) -> Option<EventResult> {
        self.dispatch_event(target_id, |node| match node {
            Self::ScrollArea { component, .. } => Some(component.on_drag(x, y, modifiers, cx)),
            Self::List { component, .. } => Some(component.on_drag(x, y, modifiers, cx)),
            Self::Tree { component, .. } => Some(component.on_drag(x, y, modifiers, cx)),
            Self::Table { component, .. } => Some(component.on_drag(x, y, modifiers, cx)),
            _ => None,
        })
    }

    /// Dispatch a drag release event to a component.
    pub fn dispatch_release_event(&self, target_id: &str, cx: &AppContext) -> Option<EventResult> {
        self.dispatch_event(target_id, |node| match node {
            Self::ScrollArea { component, .. } => Some(component.on_release(cx)),
            Self::List { component, .. } => Some(component.on_release(cx)),
            Self::Tree { component, .. } => Some(component.on_release(cx)),
            Self::Table { component, .. } => Some(component.on_release(cx)),
            _ => None,
        })
    }

    /// Dispatch a key event to a component.
    pub fn dispatch_key_event(
        &self,
        target_id: &str,
        key: &KeyCombo,
        cx: &AppContext,
    ) -> Option<EventResult> {
        self.dispatch_event(target_id, |node| match node {
            Self::Input {
                component: Some(component),
                ..
            } => Some(component.on_key(key, cx)),
            Self::Input {
                component: None, ..
            } => Some(EventResult::Ignored),
            Self::ScrollArea { component, .. } => Some(component.on_key(key, cx)),
            Self::List { component, .. } => Some(component.on_key(key, cx)),
            Self::Tree { component, .. } => Some(component.on_key(key, cx)),
            Self::Table { component, .. } => Some(component.on_key(key, cx)),
            _ => None,
        })
    }

    /// Dispatch a hover event to a component.
    ///
    /// Called when the mouse moves over a component's bounds.
    pub fn dispatch_hover_event(
        &self,
        target_id: &str,
        x: u16,
        y: u16,
        cx: &AppContext,
    ) -> Option<EventResult> {
        self.dispatch_event(target_id, |node| match node {
            Self::List { component, .. } => Some(component.on_hover(x, y, cx)),
            Self::Tree { component, .. } => Some(component.on_hover(x, y, cx)),
            Self::Table { component, .. } => Some(component.on_hover(x, y, cx)),
            _ => None,
        })
    }

    /// Calculate intrinsic width of this node
    pub fn intrinsic_width(&self) -> u16 {
        match self {
            Self::Empty => 0,
            Self::Text { content, .. } => {
                // Max line width, not total length
                content.lines().map(|l| l.len()).max().unwrap_or(0) as u16
            }
            Self::Column {
                children, layout, ..
            } => {
                let (chrome_h, _) = layout.chrome_size();
                let max_child = children
                    .iter()
                    .map(|c| c.intrinsic_width())
                    .max()
                    .unwrap_or(0);
                max_child + chrome_h
            }
            Self::Row {
                children, layout, ..
            } => {
                let (chrome_h, _) = layout.chrome_size();
                let child_sum: u16 = children.iter().map(|c| c.intrinsic_width()).sum();
                let gaps = if children.len() > 1 {
                    layout.gap * (children.len() as u16 - 1)
                } else {
                    0
                };
                child_sum + gaps + chrome_h
            }
            Self::Stack {
                children, layout, ..
            } => {
                let (chrome_h, _) = layout.chrome_size();
                let max_child = children
                    .iter()
                    .map(|c| c.intrinsic_width())
                    .max()
                    .unwrap_or(0);
                max_child + chrome_h
            }
            Self::Input {
                value, placeholder, ..
            } => {
                let content_len = if value.is_empty() {
                    placeholder.len()
                } else {
                    value.len()
                };
                (content_len + 5).max(15) as u16
            }
            Self::Button { label, .. } => (label.len() + 4) as u16,
            Self::ScrollArea { child, layout, .. } => {
                let (chrome_h, _) = layout.chrome_size();
                // ScrollArea reports child's intrinsic size (may be larger than viewport)
                child.intrinsic_width() + chrome_h
            }
            Self::List { layout, .. } | Self::Tree { layout, .. } => {
                // List/Tree width is determined by layout, not content
                let (chrome_h, _) = layout.chrome_size();
                40 + chrome_h // Default width, will be overridden by layout
            }
            Self::Table {
                layout, component, ..
            } => {
                let (chrome_h, _) = layout.chrome_size();
                // Table total width is sum of column widths
                component.total_width() + chrome_h
            }
        }
    }

    /// Calculate intrinsic height of this node
    pub fn intrinsic_height(&self) -> u16 {
        match self {
            Self::Empty => 0,
            Self::Text { content, .. } => content.lines().count().max(1) as u16,
            Self::Column {
                children, layout, ..
            } => {
                let (_, chrome_v) = layout.chrome_size();
                let child_sum: u16 = children.iter().map(|c| c.intrinsic_height()).sum();
                let gaps = if children.len() > 1 {
                    layout.gap * (children.len() as u16 - 1)
                } else {
                    0
                };
                child_sum + gaps + chrome_v
            }
            Self::Row {
                children, layout, ..
            } => {
                let (_, chrome_v) = layout.chrome_size();
                let max_child = children
                    .iter()
                    .map(|c| c.intrinsic_height())
                    .max()
                    .unwrap_or(0);
                max_child + chrome_v
            }
            Self::Stack {
                children, layout, ..
            } => {
                let (_, chrome_v) = layout.chrome_size();
                let max_child = children
                    .iter()
                    .map(|c| c.intrinsic_height())
                    .max()
                    .unwrap_or(0);
                max_child + chrome_v
            }
            Self::Input { .. } | Self::Button { .. } => 1,
            Self::ScrollArea { child, layout, .. } => {
                let (_, chrome_v) = layout.chrome_size();
                // ScrollArea reports child's intrinsic size (may be larger than viewport)
                child.intrinsic_height() + chrome_v
            }
            Self::List {
                layout, component, ..
            } => {
                let (_, chrome_v) = layout.chrome_size();
                // Total height of all items
                component.total_height() + chrome_v
            }
            Self::Tree {
                layout, component, ..
            } => {
                let (_, chrome_v) = layout.chrome_size();
                // Total height of all visible nodes
                component.total_height() + chrome_v
            }
            Self::Table {
                layout, component, ..
            } => {
                let (_, chrome_v) = layout.chrome_size();
                // Total height of all rows plus header
                component.total_height() + 1 + chrome_v // +1 for header row
            }
        }
    }
}
