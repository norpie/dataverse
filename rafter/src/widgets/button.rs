//! Button widget.

use tuidom::{Element, Style, Transitions};

use crate::{HandlerRegistry, WidgetHandlers};

/// A button widget builder.
///
/// This is a stateless widget that creates a clickable button element.
///
/// # Example
///
/// ```ignore
/// // In page! macro:
/// button (label: "Click me", id: "my-btn")
///     style (bg: primary)
///     on_activate: my_handler()
///
/// // Disabled button:
/// button (label: "Loading...", id: "btn", disabled)
///     style (bg: muted)
/// ```
#[derive(Clone, Debug, Default)]
pub struct Button {
    label: Option<String>,
    id: Option<String>,
    disabled: bool,
    style: Option<Style>,
    transitions: Option<Transitions>,
}

impl Button {
    /// Create a new button builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the button label.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Set the button id.
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Mark the button as disabled.
    ///
    /// Disabled buttons are not focusable, not clickable, and don't register handlers.
    pub fn disabled(mut self) -> Self {
        self.disabled = true;
        self
    }

    /// Set the button style.
    pub fn style(mut self, style: Style) -> Self {
        self.style = Some(style);
        self
    }

    /// Set the button transitions.
    pub fn transitions(mut self, transitions: Transitions) -> Self {
        self.transitions = Some(transitions);
        self
    }

    /// Build the button element.
    ///
    /// Registers the `on_activate` handler if provided and not disabled.
    pub fn build(self, registry: &HandlerRegistry, handlers: &WidgetHandlers) -> Element {
        let label = self.label.unwrap_or_default();
        let id = self.id.unwrap_or_else(|| "button".into());

        let mut elem = Element::text(&label)
            .id(&id)
            .focusable(!self.disabled)
            .clickable(!self.disabled);

        if let Some(style) = self.style {
            elem = elem.style(style);
        }
        if let Some(transitions) = self.transitions {
            elem = elem.transitions(transitions);
        }

        // Only register handler if not disabled
        if !self.disabled {
            if let Some(handler) = handlers.get("on_activate") {
                registry.register(&id, "on_activate", handler.clone());
            }
        }

        elem
    }
}
