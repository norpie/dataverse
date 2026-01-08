//! Checkbox widget - a toggleable checkbox with optional label.

use tuidom::{Element, Style, Transitions};

use crate::{HandlerRegistry, State, WidgetHandlers};

/// Checkbox display variant.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CheckboxVariant {
    /// Large checkbox: [x] or [ ]
    #[default]
    Big,
    /// Small checkbox: ▣ or □
    Small,
}

/// Typestate marker: checkbox needs a state reference.
pub struct NeedsState;

/// Typestate marker: checkbox has a state reference.
pub struct HasState<'a>(&'a State<bool>);

/// A checkbox widget builder.
///
/// Uses typestate pattern to enforce `state()` is called before `build()`.
///
/// # Example
///
/// ```ignore
/// // In page! macro:
/// checkbox (state: self.agree, id: "agree", label: "I agree to terms", small)
///     style (fg: primary)
///     on_change: agreement_changed()
/// ```
#[derive(Clone, Debug)]
pub struct Checkbox<S = NeedsState> {
    state_marker: S,
    id: Option<String>,
    label: Option<String>,
    variant: CheckboxVariant,
    disabled: bool,
    style: Option<Style>,
    style_focused: Option<Style>,
    style_disabled: Option<Style>,
    label_style: Option<Style>,
    transitions: Option<Transitions>,
}

impl Default for Checkbox<NeedsState> {
    fn default() -> Self {
        Self::new()
    }
}

impl Checkbox<NeedsState> {
    /// Create a new checkbox builder.
    pub fn new() -> Self {
        Self {
            state_marker: NeedsState,
            id: None,
            label: None,
            variant: CheckboxVariant::default(),
            disabled: false,
            style: None,
            style_focused: None,
            style_disabled: None,
            label_style: None,
            transitions: None,
        }
    }

    /// Set the state reference. Required before calling `build()`.
    pub fn state(self, s: &State<bool>) -> Checkbox<HasState<'_>> {
        Checkbox {
            state_marker: HasState(s),
            id: self.id,
            label: self.label,
            variant: self.variant,
            disabled: self.disabled,
            style: self.style,
            style_focused: self.style_focused,
            style_disabled: self.style_disabled,
            label_style: self.label_style,
            transitions: self.transitions,
        }
    }
}

impl<S> Checkbox<S> {
    /// Set the checkbox id.
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Set the checkbox label.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Set the checkbox variant.
    pub fn variant(mut self, v: CheckboxVariant) -> Self {
        self.variant = v;
        self
    }

    /// Use small variant (✓/○).
    pub fn small(mut self) -> Self {
        self.variant = CheckboxVariant::Small;
        self
    }

    /// Use big variant ([x]/[ ]).
    pub fn big(mut self) -> Self {
        self.variant = CheckboxVariant::Big;
        self
    }

    /// Mark the checkbox as disabled.
    pub fn disabled(mut self) -> Self {
        self.disabled = true;
        self
    }

    /// Set the checkbox style (applies to the checkbox indicator).
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

impl<'a> Checkbox<HasState<'a>> {
    /// Build the checkbox element.
    ///
    /// Registers the `on_change` handler if provided and not disabled.
    pub fn build(self, registry: &HandlerRegistry, handlers: &WidgetHandlers) -> Element {
        let state = self.state_marker.0;
        let checked = state.get();
        let id = self.id.clone().unwrap_or_else(|| "checkbox".into());

        // Build checkbox indicator text
        let indicator = match self.variant {
            CheckboxVariant::Big => {
                if checked {
                    "[x]"
                } else {
                    "[ ]"
                }
            }
            CheckboxVariant::Small => {
                if checked {
                    "◼"
                } else {
                    "◻"
                }
            }
        };

        // Build the checkbox indicator element
        let mut checkbox_elem = Element::text(indicator);
        if let Some(style) = self.style.clone() {
            checkbox_elem = checkbox_elem.style(style);
        }

        // Build the full element (indicator + optional label)
        let mut elem = if let Some(label_text) = &self.label {
            let mut label_elem = Element::text(label_text);
            if let Some(label_style) = self.label_style.clone() {
                label_elem = label_elem.style(label_style);
            }

            Element::row()
                .gap(1)
                .children(vec![checkbox_elem, label_elem])
        } else {
            checkbox_elem
        };

        elem = elem
            .id(&id)
            .focusable(!self.disabled)
            .clickable(!self.disabled)
            .disabled(self.disabled);

        if let Some(style) = self.style_focused {
            elem = elem.style_focused(style);
        }
        if let Some(style) = self.style_disabled {
            elem = elem.style_disabled(style);
        }
        if let Some(transitions) = self.transitions {
            elem = elem.transitions(transitions);
        }

        // Register toggle handler if not disabled
        if !self.disabled {
            if let Some(on_change) = handlers.get("on_change").cloned() {
                let state_clone = state.clone();
                registry.register(
                    &id,
                    "on_activate",
                    std::sync::Arc::new(move |hx| {
                        state_clone.update(|v| *v = !*v);
                        on_change(hx);
                    }),
                );
            } else {
                // Toggle without user callback
                let state_clone = state.clone();
                registry.register(
                    &id,
                    "on_activate",
                    std::sync::Arc::new(move |_hx| {
                        state_clone.update(|v| *v = !*v);
                    }),
                );
            }
        }

        elem
    }
}
