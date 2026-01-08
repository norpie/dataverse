//! Select widget - a dropdown selection component.

use std::sync::Arc;

use tuidom::{Element, Style, Transitions};

use crate::state::State;
use crate::{HandlerRegistry, WidgetHandlers};

/// State for a select widget.
///
/// Contains the dropdown open/closed state, selected value, and available options.
///
/// # Example
///
/// ```ignore
/// // In app struct (will be wrapped in State<> by #[app] macro):
/// country: SelectState<String>,
///
/// // Initialize in on_start:
/// self.country.set(SelectState::new([
///     ("us".to_string(), "United States"),
///     ("uk".to_string(), "United Kingdom"),
///     ("de".to_string(), "Germany"),
/// ]));
/// ```
#[derive(Clone, Debug)]
pub struct SelectState<T: Clone> {
    /// Whether the dropdown is open.
    pub open: bool,
    /// The currently selected value, if any.
    pub value: Option<T>,
    /// Available options as (value, label) pairs.
    pub options: Vec<(T, String)>,
}

impl<T: Clone> Default for SelectState<T> {
    fn default() -> Self {
        Self {
            open: false,
            value: None,
            options: Vec::new(),
        }
    }
}

impl<T: Clone> SelectState<T> {
    /// Create a new SelectState with the given options.
    pub fn new(options: impl IntoIterator<Item = (T, impl Into<String>)>) -> Self {
        Self {
            open: false,
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

/// Typestate marker: select needs a state reference.
pub struct NeedsState;

/// Typestate marker: select has a state reference.
pub struct HasState<'a, T: Clone>(&'a State<SelectState<T>>);

/// A select widget builder.
///
/// Uses typestate pattern to enforce `state()` is called before `build()`.
///
/// # Example
///
/// ```ignore
/// // In page! macro:
/// select (state: self.country, id: "country", placeholder: "Choose country...")
///     style (bg: surface)
///     on_change: country_changed()
/// ```
#[derive(Clone, Debug)]
pub struct Select<S = NeedsState> {
    state_marker: S,
    id: Option<String>,
    placeholder: Option<String>,
    disabled: bool,
    style: Option<Style>,
    style_focused: Option<Style>,
    style_disabled: Option<Style>,
    transitions: Option<Transitions>,
}

impl Default for Select<NeedsState> {
    fn default() -> Self {
        Self::new()
    }
}

impl Select<NeedsState> {
    /// Create a new select builder.
    pub fn new() -> Self {
        Self {
            state_marker: NeedsState,
            id: None,
            placeholder: None,
            disabled: false,
            style: None,
            style_focused: None,
            style_disabled: None,
            transitions: None,
        }
    }

    /// Set the state reference. Required before calling `build()`.
    pub fn state<T: Clone + PartialEq + Send + Sync + 'static>(
        self,
        s: &State<SelectState<T>>,
    ) -> Select<HasState<'_, T>> {
        Select {
            state_marker: HasState(s),
            id: self.id,
            placeholder: self.placeholder,
            disabled: self.disabled,
            style: self.style,
            style_focused: self.style_focused,
            style_disabled: self.style_disabled,
            transitions: self.transitions,
        }
    }
}

impl<S> Select<S> {
    /// Set the select id.
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Set the placeholder text shown when no value is selected.
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    /// Mark the select as disabled.
    pub fn disabled(mut self) -> Self {
        self.disabled = true;
        self
    }

    /// Set the select style.
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

impl<'a, T: Clone + PartialEq + Send + Sync + 'static> Select<HasState<'a, T>> {
    /// Build the select element.
    ///
    /// Registers the toggle and option handlers if not disabled.
    pub fn build(self, registry: &HandlerRegistry, handlers: &WidgetHandlers) -> Element {
        let state = self.state_marker.0;
        let current = state.get();
        let id = self.id.clone().unwrap_or_else(|| "select".into());

        log::debug!(
            "Select::build id={} open={} options_count={}",
            id,
            current.open,
            current.options.len()
        );

        // Display text: selected label or placeholder
        let display_text = current
            .value
            .as_ref()
            .and_then(|v| current.options.iter().find(|(ov, _)| ov == v))
            .map(|(_, label)| label.clone())
            .unwrap_or_else(|| {
                self.placeholder
                    .clone()
                    .unwrap_or_else(|| "Select...".into())
            });

        // Build toggle row
        let arrow = if current.open { "▲" } else { "▼" };
        let mut toggle = Element::row()
            .id(&id)
            .gap(1)
            .focusable(!self.disabled)
            .clickable(!self.disabled)
            .disabled(self.disabled)
            .children(vec![Element::text(&display_text), Element::text(arrow)]);

        if let Some(style) = self.style.clone() {
            toggle = toggle.style(style);
        }
        if let Some(style) = self.style_focused.clone() {
            toggle = toggle.style_focused(style);
        }
        if let Some(style) = self.style_disabled.clone() {
            toggle = toggle.style_disabled(style);
        }
        if let Some(transitions) = self.transitions.clone() {
            toggle = toggle.transitions(transitions);
        }

        // Register toggle handler
        if !self.disabled {
            let state_clone = state.clone();
            registry.register(
                &id,
                "on_activate",
                Arc::new(move |_hx| {
                    state_clone.update(|s| s.open = !s.open);
                }),
            );
        }

        // Build dropdown if open
        if current.open {
            log::debug!("Select::build rendering {} options", current.options.len());
            let mut options_col = Element::col();

            for (i, (value, label)) in current.options.iter().enumerate() {
                let opt_id = format!("{}-opt-{}", id, i);
                let is_selected = current.value.as_ref() == Some(value);

                let mut opt_elem = Element::text(label)
                    .id(&opt_id)
                    .focusable(true)
                    .clickable(true);

                // Highlight selected option
                if is_selected {
                    opt_elem = opt_elem.style(Style::new().bold());
                }

                options_col = options_col.child(opt_elem);

                // Register option handler
                let state_clone = state.clone();
                let value_clone = value.clone();
                let on_change = handlers.get("on_change").cloned();
                registry.register(
                    &opt_id,
                    "on_activate",
                    Arc::new(move |hx| {
                        state_clone.update(|s| {
                            s.value = Some(value_clone.clone());
                            s.open = false;
                        });
                        if let Some(ref handler) = on_change {
                            handler(hx);
                        }
                    }),
                );
            }

            // Use absolute positioning for dropdown overlay
            use tuidom::{Position, Size};

            let dropdown = options_col
                .position(Position::Absolute)
                .top(1) // Below the toggle
                .left(0)
                .z_index(100) // Render above other content
                .style(Style::new().background(tuidom::Color::var("surface")));

            Element::box_()
                .height(Size::Fixed(1)) // Only take toggle's height
                .child(toggle)
                .child(dropdown)
        } else {
            toggle
        }
    }
}
