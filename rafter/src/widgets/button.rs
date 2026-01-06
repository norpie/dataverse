//! Button widget.
//!
//! A clickable button that stores a HandlerId for its click event.

use tuidom::Element;

use crate::HandlerId;

/// A button widget builder.
///
/// # Example
///
/// ```ignore
/// // In page! macro:
/// button (label: "Click me", id: "my-btn") on_click: my_handler()
///
/// // Generates:
/// button::new()
///     .label("Click me")
///     .id("my-btn")
///     .on_click_id(HandlerId::new("my_handler"))
///     .element()
/// ```
#[derive(Clone, Debug, Default)]
pub struct Button {
    label: String,
    id: Option<String>,
    on_click: Option<HandlerId>,
}

impl Button {
    /// Create a new button builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the button label.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    /// Set the button id.
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Set the click handler ID.
    ///
    /// When the button is clicked, the runtime will dispatch to the handler
    /// with this ID through the app/modal's dispatch method.
    pub fn on_click_id(mut self, handler: HandlerId) -> Self {
        self.on_click = Some(handler);
        self
    }

    /// Get the click handler ID if set.
    pub fn click_handler(&self) -> Option<&HandlerId> {
        self.on_click.as_ref()
    }

    /// Build the button element.
    ///
    /// Returns a tuidom Element that represents this button.
    pub fn element(self) -> Element {
        let mut elem = Element::text(&self.label);

        if let Some(id) = &self.id {
            elem = elem.id(id);
        }

        // Mark as focusable and clickable
        elem = elem.focusable(true).clickable(true);

        // Store the handler ID as data on the element for runtime dispatch
        if let Some(handler) = &self.on_click {
            elem = elem.data("on_click", handler.0.clone());
        }

        elem
    }
}

/// Create a new button builder.
///
/// This is a convenience function for `Button::new()`.
pub fn new() -> Button {
    Button::new()
}
