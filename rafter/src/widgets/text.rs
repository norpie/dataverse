//! Text widget - simple text display.

use tuidom::{Element, Style, Transitions};

use crate::{HandlerRegistry, WidgetHandlers};

/// A text display widget builder.
///
/// This is a stateless widget that simply displays text.
///
/// # Example
///
/// ```ignore
/// // In page! macro:
/// text (content: "Hello world", id: "greeting")
///     style (fg: primary)
/// ```
#[derive(Clone, Debug, Default)]
pub struct Text {
    content: Option<String>,
    id: Option<String>,
    style: Option<Style>,
    transitions: Option<Transitions>,
}

impl Text {
    /// Create a new text widget builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the text content.
    pub fn content(mut self, content: impl Into<String>) -> Self {
        self.content = Some(content.into());
        self
    }

    /// Set the element id.
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Set the style.
    pub fn style(mut self, style: Style) -> Self {
        self.style = Some(style);
        self
    }

    /// Set transitions.
    pub fn transitions(mut self, transitions: Transitions) -> Self {
        self.transitions = Some(transitions);
        self
    }

    /// Build the text element.
    ///
    /// Text is a stateless widget, so it doesn't use the registry or handlers,
    /// but accepts them for API consistency with other widgets.
    pub fn build(self, _registry: &HandlerRegistry, _handlers: &WidgetHandlers) -> Element {
        let content = self.content.unwrap_or_default();
        let mut elem = Element::text(&content);

        if let Some(id) = self.id {
            elem = elem.id(id);
        }
        if let Some(style) = self.style {
            elem = elem.style(style);
        }
        if let Some(transitions) = self.transitions {
            elem = elem.transitions(transitions);
        }

        elem
    }
}
