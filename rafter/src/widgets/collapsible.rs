//! Collapsible widget - expandable/collapsible content section.

use std::sync::Arc;

use tuidom::{Element, Style, Transitions};

use crate::state::State;
use crate::{HandlerRegistry, WidgetHandlers};

/// Typestate marker: collapsible needs a state reference.
pub struct NeedsState;

/// Typestate marker: collapsible has a state reference.
pub struct HasState<'a>(&'a State<bool>);

/// A collapsible widget builder.
///
/// Shows a header that can be clicked to expand/collapse content.
///
/// # Example
///
/// ```ignore
/// // In page! macro:
/// collapsible (state: self.details_open, id: "details", header: "Show Details") {
///     text (content: "These are the details...")
///     text (content: "More information here.")
/// }
/// ```
#[derive(Clone, Debug)]
pub struct Collapsible<S = NeedsState> {
    state_marker: S,
    id: Option<String>,
    header: String,
    children: Vec<Element>,
    style: Option<Style>,
    style_focused: Option<Style>,
    header_style: Option<Style>,
    content_style: Option<Style>,
    transitions: Option<Transitions>,
}

impl Default for Collapsible<NeedsState> {
    fn default() -> Self {
        Self::new()
    }
}

impl Collapsible<NeedsState> {
    /// Create a new collapsible builder.
    pub fn new() -> Self {
        Self {
            state_marker: NeedsState,
            id: None,
            header: "Toggle".into(),
            children: Vec::new(),
            style: None,
            style_focused: None,
            header_style: None,
            content_style: None,
            transitions: None,
        }
    }

    /// Set the state reference. Required before calling `build()`.
    pub fn state(self, s: &State<bool>) -> Collapsible<HasState<'_>> {
        Collapsible {
            state_marker: HasState(s),
            id: self.id,
            header: self.header,
            children: self.children,
            style: self.style,
            style_focused: self.style_focused,
            header_style: self.header_style,
            content_style: self.content_style,
            transitions: self.transitions,
        }
    }
}

impl<S> Collapsible<S> {
    /// Set the collapsible id.
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Set the header text.
    pub fn header(mut self, header: impl Into<String>) -> Self {
        self.header = header.into();
        self
    }

    /// Set the collapsible children (content when expanded).
    pub fn children(mut self, children: Vec<Element>) -> Self {
        self.children = children;
        self
    }

    /// Add a single child.
    pub fn child(mut self, child: Element) -> Self {
        self.children.push(child);
        self
    }

    /// Set the container style.
    pub fn style(mut self, s: Style) -> Self {
        self.style = Some(s);
        self
    }

    /// Set the style when focused.
    pub fn style_focused(mut self, s: Style) -> Self {
        self.style_focused = Some(s);
        self
    }

    /// Set the header row style.
    pub fn header_style(mut self, s: Style) -> Self {
        self.header_style = Some(s);
        self
    }

    /// Set the content container style.
    pub fn content_style(mut self, s: Style) -> Self {
        self.content_style = Some(s);
        self
    }

    /// Set transitions.
    pub fn transitions(mut self, t: Transitions) -> Self {
        self.transitions = Some(t);
        self
    }
}

impl<'a> Collapsible<HasState<'a>> {
    /// Build the collapsible element.
    pub fn build(self, registry: &HandlerRegistry, _handlers: &WidgetHandlers) -> Element {
        let state = self.state_marker.0;
        let expanded = state.get();
        let id = self.id.clone().unwrap_or_else(|| "collapsible".into());
        let header_id = format!("{}-header", id);

        // Arrow indicator
        let arrow = if expanded { "▼" } else { "▶" };

        // Build header row
        let mut header_row = Element::row()
            .id(&header_id)
            .gap(1)
            .focusable(true)
            .clickable(true)
            .children(vec![
                Element::text(arrow),
                Element::text(&self.header),
            ]);

        if let Some(style) = self.header_style {
            header_row = header_row.style(style);
        }
        if let Some(style) = self.style_focused.clone() {
            header_row = header_row.style_focused(style);
        }

        // Register toggle handler
        let state_clone = state.clone();
        registry.register(
            &header_id,
            "on_activate",
            Arc::new(move |_hx| {
                state_clone.update(|v| *v = !*v);
            }),
        );

        // Build container
        let mut container = Element::col().id(&id);

        if let Some(style) = self.style {
            container = container.style(style);
        }
        if let Some(transitions) = self.transitions {
            container = container.transitions(transitions);
        }

        container = container.child(header_row);

        // Add content if expanded
        if expanded {
            let mut content = Element::col()
                .padding(tuidom::Edges::left(2)) // Indent content
                .children(self.children);

            if let Some(style) = self.content_style {
                content = content.style(style);
            }

            container = container.child(content);
        }

        container
    }
}
