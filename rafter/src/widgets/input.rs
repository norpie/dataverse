//! Text input widget - a single-line text input field.

use std::sync::Arc;

use tuidom::{Element, Style, Transitions};

use crate::state::State;
use crate::{HandlerRegistry, WidgetHandlers};

/// Typestate marker: input needs a state reference.
pub struct NeedsState;

/// Typestate marker: input has a state reference.
pub struct HasState<'a>(&'a State<String>);

/// A text input widget builder.
///
/// Uses typestate pattern to enforce `state()` is called before `build()`.
///
/// # Example
///
/// ```ignore
/// // In page! macro:
/// input (state: self.username, id: "username", placeholder: "Enter username...")
///     style (bg: surface)
///     on_change: username_changed()
///     on_submit: login()
/// ```
#[derive(Clone, Debug)]
pub struct Input<S = NeedsState> {
    state_marker: S,
    id: Option<String>,
    placeholder: Option<String>,
    disabled: bool,
    width: Option<u16>,
    style: Option<Style>,
    style_focused: Option<Style>,
    style_disabled: Option<Style>,
    transitions: Option<Transitions>,
}

impl Default for Input<NeedsState> {
    fn default() -> Self {
        Self::new()
    }
}

impl Input<NeedsState> {
    /// Create a new input builder.
    pub fn new() -> Self {
        Self {
            state_marker: NeedsState,
            id: None,
            placeholder: None,
            disabled: false,
            width: None,
            style: None,
            style_focused: None,
            style_disabled: None,
            transitions: None,
        }
    }

    /// Set the state reference. Required before calling `build()`.
    pub fn state(self, s: &State<String>) -> Input<HasState<'_>> {
        Input {
            state_marker: HasState(s),
            id: self.id,
            placeholder: self.placeholder,
            disabled: self.disabled,
            width: self.width,
            style: self.style,
            style_focused: self.style_focused,
            style_disabled: self.style_disabled,
            transitions: self.transitions,
        }
    }
}

impl<S> Input<S> {
    /// Set the input id.
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Set the placeholder text.
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    /// Mark the input as disabled.
    pub fn disabled(mut self) -> Self {
        self.disabled = true;
        self
    }

    /// Set the input width in characters.
    pub fn width(mut self, width: u16) -> Self {
        self.width = Some(width);
        self
    }

    /// Set the input style.
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
}

impl<'a> Input<HasState<'a>> {
    /// Build the input element.
    ///
    /// Registers `on_change` and `on_submit` handlers if provided and not disabled.
    pub fn build(self, registry: &HandlerRegistry, handlers: &WidgetHandlers) -> Element {
        let state = self.state_marker.0;
        let value = state.get();
        let id = self.id.clone().unwrap_or_else(|| "input".into());

        // Build the text input element
        let mut elem = Element::text_input(&value)
            .id(&id)
            .focusable(!self.disabled)
            .captures_input(!self.disabled)
            .disabled(self.disabled);

        // Set width - use Fixed if specified, otherwise Fill
        elem = match self.width {
            Some(w) => elem.width(tuidom::Size::Fixed(w)),
            None => elem.width(tuidom::Size::Fill),
        };

        if let Some(placeholder) = &self.placeholder {
            elem = elem.placeholder(placeholder);
        }

        if let Some(style) = self.style.clone() {
            elem = elem.style(style);
        }

        if let Some(style) = self.style_focused.clone() {
            elem = elem.style_focused(style);
        }

        if let Some(style) = self.style_disabled.clone() {
            elem = elem.style_disabled(style);
        }

        if let Some(transitions) = self.transitions.clone() {
            elem = elem.transitions(transitions);
        }

        // Register handlers if not disabled
        if !self.disabled {
            // Always register on_change to update State<String>
            let state_clone = state.clone();
            let user_handler = handlers.get("on_change").cloned();
            registry.register(
                &id,
                "on_change",
                Arc::new(move |hx| {
                    // First update State<String> from event data
                    if let Some(text) = hx.changed_text() {
                        state_clone.set(text.to_string());
                    }
                    // Then call user's handler if provided
                    if let Some(ref handler) = user_handler {
                        handler(hx);
                    }
                }),
            );

            if let Some(on_submit) = handlers.get("on_submit").cloned() {
                registry.register(&id, "on_submit", on_submit);
            }
        }

        elem
    }
}
