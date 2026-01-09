//! List widget - a selectable list of items.

use std::hash::Hash;
use std::sync::Arc;

use tuidom::{Color, Element, Style, Transitions};

use crate::state::State;
use crate::{HandlerRegistry, WidgetHandlers};

use super::selection::{Selection, SelectionMode};

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
}

/// State for a List widget.
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
    /// The key of the last activated item. Set before handlers are called.
    pub last_activated: Option<T::Key>,
}

impl<T: ListItem> Default for ListState<T> {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            selection: Selection::none(),
            last_activated: None,
        }
    }
}

impl<T: ListItem> ListState<T> {
    /// Create a new ListState with the given items.
    pub fn new(items: Vec<T>) -> Self {
        Self {
            items,
            selection: Selection::none(),
            last_activated: None,
        }
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
}

/// Typestate marker: list needs a state reference.
pub struct NeedsState;

/// Typestate marker: list has a state reference.
pub struct HasListState<'a, T: ListItem>(&'a State<ListState<T>>);

/// A list widget builder.
///
/// Uses typestate pattern to enforce `state()` is called before `build()`.
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
}

impl<'a, T: ListItem> List<HasListState<'a, T>> {
    /// Build the list element.
    ///
    /// Creates a column of focusable rows, one per item. Each row can be
    /// activated to toggle selection.
    pub fn build(self, registry: &HandlerRegistry, handlers: &WidgetHandlers) -> Element {
        let state = self.state_marker.0;
        let current = state.get();
        let list_id = self.id.clone().unwrap_or_else(|| "list".into());

        let mut rows = Vec::with_capacity(current.items.len());

        for item in &current.items {
            let key = item.key();
            let row_id = format!("{}-item-{}", list_id, key.to_string());
            let is_selected = current.selection.is_selected(&key);

            // Build row element with item's rendered content
            let mut row = Element::row()
                .id(&row_id)
                .width(tuidom::Size::Fill)
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

            rows.push(row);

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
        }

        // Build container column
        let mut container = Element::col()
            .id(&list_id)
            .width(tuidom::Size::Fill)
            .children(rows);

        if let Some(style) = self.style {
            container = container.style(style);
        }
        if let Some(transitions) = self.transitions {
            container = container.transitions(transitions);
        }

        container
    }
}
