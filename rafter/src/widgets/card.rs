//! Card widget - a container with optional header styling.

use tuidom::{Element, Style, Transitions};

use crate::{HandlerRegistry, WidgetHandlers};

/// A card container widget builder.
///
/// Cards are simple containers that group related content together.
/// They support styling for the container and can hold arbitrary children.
///
/// # Example
///
/// ```ignore
/// // In page! macro:
/// card (id: "user-card")
///     style (bg: surface, padding: 2)
/// {
///     text (content: "User Profile") style (bold)
///     text (content: "John Doe")
///     button (label: "Edit", id: "edit") on_activate: edit_profile()
/// }
/// ```
#[derive(Clone, Debug, Default)]
pub struct Card {
    id: Option<String>,
    children: Vec<Element>,
    style: Option<Style>,
    style_focused: Option<Style>,
    style_disabled: Option<Style>,
    transitions: Option<Transitions>,
}

impl Card {
    /// Create a new card builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the card id.
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Set the card's children.
    pub fn children(mut self, children: Vec<Element>) -> Self {
        self.children = children;
        self
    }

    /// Add a single child to the card.
    pub fn child(mut self, child: Element) -> Self {
        self.children.push(child);
        self
    }

    /// Set the card style.
    pub fn style(mut self, s: Style) -> Self {
        self.style = Some(s);
        self
    }

    /// Set the style when focused.
    pub fn style_focused(mut self, s: Style) -> Self {
        self.style_focused = Some(s);
        self
    }

    /// Set the style when disabled.
    pub fn style_disabled(mut self, s: Style) -> Self {
        self.style_disabled = Some(s);
        self
    }

    /// Set transitions.
    pub fn transitions(mut self, t: Transitions) -> Self {
        self.transitions = Some(t);
        self
    }

    /// Build the card element.
    pub fn build(self, _registry: &HandlerRegistry, _handlers: &WidgetHandlers) -> Element {
        let id = self.id.unwrap_or_else(|| "card".into());

        let mut elem = Element::col().id(&id).children(self.children);

        if let Some(style) = self.style {
            elem = elem.style(style);
        }
        if let Some(style) = self.style_focused {
            elem = elem.style_focused(style);
        }
        if let Some(style) = self.style_disabled {
            elem = elem.style_disabled(style);
        }
        if let Some(transitions) = self.transitions {
            elem = elem.transitions(transitions);
        }

        elem
    }
}
