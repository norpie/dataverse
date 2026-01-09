//! RadioGroup widget - a group of mutually exclusive radio buttons.

use std::sync::Arc;

use tuidom::{Color, Element, Style, Transitions};

use crate::state::State;
use crate::{HandlerRegistry, WidgetHandlers};

/// State for a radio group widget.
///
/// Contains the selected value and available options.
///
/// # Example
///
/// ```ignore
/// // In app struct (will be wrapped in State<> by #[app] macro):
/// priority: RadioState<String>,
///
/// // Initialize in on_start:
/// self.priority.set(RadioState::new([
///     ("low".to_string(), "Low"),
///     ("medium".to_string(), "Medium"),
///     ("high".to_string(), "High"),
/// ]));
/// ```
#[derive(Clone, Debug)]
pub struct RadioState<T: Clone> {
    /// The currently selected value, if any.
    pub value: Option<T>,
    /// Available options as (value, label) pairs.
    pub options: Vec<(T, String)>,
}

impl<T: Clone> Default for RadioState<T> {
    fn default() -> Self {
        Self {
            value: None,
            options: Vec::new(),
        }
    }
}

impl<T: Clone> RadioState<T> {
    /// Create a new RadioState with the given options.
    pub fn new(options: impl IntoIterator<Item = (T, impl Into<String>)>) -> Self {
        Self {
            value: None,
            options: options.into_iter().map(|(v, l)| (v, l.into())).collect(),
        }
    }

    /// Set the initial selected value.
    pub fn with_value(mut self, value: T) -> Self {
        self.value = Some(value);
        self
    }
}

/// Typestate marker: radio group needs a state reference.
pub struct NeedsState;

/// Typestate marker: radio group has a state reference.
pub struct HasState<'a, T: Clone>(&'a State<RadioState<T>>);

/// A radio group widget builder.
///
/// Uses typestate pattern to enforce `state()` is called before `build()`.
///
/// # Example
///
/// ```ignore
/// // In page! macro:
/// radio_group (state: self.priority, id: "priority")
///     style (bg: surface)
///     on_change: priority_changed()
/// ```
#[derive(Clone, Debug)]
pub struct RadioGroup<S = NeedsState> {
    state_marker: S,
    id: Option<String>,
    disabled: bool,
    style: Option<Style>,
    style_focused: Option<Style>,
    style_disabled: Option<Style>,
    option_style: Option<Style>,
    label_style: Option<Style>,
    transitions: Option<Transitions>,
}

impl Default for RadioGroup<NeedsState> {
    fn default() -> Self {
        Self::new()
    }
}

impl RadioGroup<NeedsState> {
    /// Create a new radio group builder.
    pub fn new() -> Self {
        Self {
            state_marker: NeedsState,
            id: None,
            disabled: false,
            style: None,
            style_focused: None,
            style_disabled: None,
            option_style: None,
            label_style: None,
            transitions: None,
        }
    }

    /// Set the state reference. Required before calling `build()`.
    pub fn state<T: Clone + PartialEq + Send + Sync + 'static>(
        self,
        s: &State<RadioState<T>>,
    ) -> RadioGroup<HasState<'_, T>> {
        RadioGroup {
            state_marker: HasState(s),
            id: self.id,
            disabled: self.disabled,
            style: self.style,
            style_focused: self.style_focused,
            style_disabled: self.style_disabled,
            option_style: self.option_style,
            label_style: self.label_style,
            transitions: self.transitions,
        }
    }
}

impl<S> RadioGroup<S> {
    /// Set the radio group id.
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Mark the radio group as disabled.
    pub fn disabled(mut self) -> Self {
        self.disabled = true;
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

    /// Set the style when disabled.
    pub fn style_disabled(mut self, s: Style) -> Self {
        self.style_disabled = Some(s);
        self
    }

    /// Set the style for each option row.
    pub fn option_style(mut self, s: Style) -> Self {
        self.option_style = Some(s);
        self
    }

    /// Set the label style.
    pub fn label_style(mut self, s: Style) -> Self {
        self.label_style = Some(s);
        self
    }

    /// Set transitions.
    pub fn transitions(mut self, t: Transitions) -> Self {
        self.transitions = Some(t);
        self
    }
}

impl<'a, T: Clone + PartialEq + Send + Sync + 'static> RadioGroup<HasState<'a, T>> {
    /// Build the radio group element.
    ///
    /// Registers handlers for each option if not disabled.
    pub fn build(self, registry: &HandlerRegistry, handlers: &WidgetHandlers) -> Element {
        let state = self.state_marker.0;
        let current = state.get();
        let id = self.id.clone().unwrap_or_else(|| "radio".into());

        let mut container = Element::col();

        if let Some(style) = self.style.clone() {
            container = container.style(style);
        }
        if let Some(transitions) = self.transitions.clone() {
            container = container.transitions(transitions);
        }

        for (i, (value, label)) in current.options.iter().enumerate() {
            let opt_id = format!("{}-opt-{}", id, i);
            let is_selected = current.value.as_ref() == Some(value);

            // Radio indicator: ● for selected, ○ for unselected
            let indicator = if is_selected { "●" } else { "○" };

            let mut indicator_elem = Element::text(indicator);
            if let Some(style) = self.option_style.clone() {
                indicator_elem = indicator_elem.style(style);
            }

            let mut label_elem = Element::text(label);
            if let Some(style) = self.label_style.clone() {
                label_elem = label_elem.style(style);
            }

            let mut opt_row = Element::row()
                .id(&opt_id)
                .gap(1)
                .focusable(!self.disabled)
                .clickable(!self.disabled)
                .disabled(self.disabled)
                .children(vec![indicator_elem, label_elem]);

            let focused_style = self
                .style_focused
                .clone()
                .unwrap_or_else(|| Style::new().background(Color::var("radio.focused")));
            let disabled_style = self
                .style_disabled
                .clone()
                .unwrap_or_else(|| Style::new().background(Color::var("radio.disabled")));
            opt_row = opt_row.style_focused(focused_style);
            opt_row = opt_row.style_disabled(disabled_style);

            container = container.child(opt_row);

            // Register option handler
            if !self.disabled {
                let state_clone = state.clone();
                let value_clone = value.clone();
                let on_change = handlers.get("on_change").cloned();
                registry.register(
                    &opt_id,
                    "on_activate",
                    Arc::new(move |hx| {
                        state_clone.update(|s| {
                            s.value = Some(value_clone.clone());
                        });
                        if let Some(ref handler) = on_change {
                            handler(hx);
                        }
                    }),
                );
            }
        }

        container.id(&id)
    }
}
