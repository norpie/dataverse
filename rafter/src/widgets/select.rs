//! Select widget - a dropdown selection component.

use std::hash::Hash;
use std::sync::Arc;

use tuidom::{Color, Element, Style, Transitions};

use super::selection::{Selection, SelectionMode};
use crate::state::State;
use crate::{HandlerRegistry, WidgetHandlers};

/// State for a select widget.
///
/// Contains the dropdown open/closed state, selected value(s), and available options.
///
/// # Example
///
/// ```ignore
/// // In app struct (will be wrapped in State<> by #[app] macro):
/// country: SelectState<String>,
///
/// // Initialize in on_start (single-select, the default):
/// self.country.set(SelectState::new([
///     ("us".to_string(), "United States"),
///     ("uk".to_string(), "United Kingdom"),
///     ("de".to_string(), "Germany"),
/// ]));
///
/// // Multi-select mode:
/// self.tags.set(SelectState::new([...]).with_selection(SelectionMode::Multi));
/// ```
#[derive(Clone, Debug)]
pub struct SelectState<T: Clone + Eq + Hash> {
    /// Whether the dropdown is open.
    pub open: bool,
    /// Selection state (supports single and multi-select).
    pub selection: Selection<T>,
    /// Available options as (value, label) pairs.
    pub options: Vec<(T, String)>,
}

impl<T: Clone + Eq + Hash> Default for SelectState<T> {
    fn default() -> Self {
        Self {
            open: false,
            selection: Selection::single(),
            options: Vec::new(),
        }
    }
}

impl<T: Clone + Eq + Hash> SelectState<T> {
    /// Create a new SelectState with the given options.
    ///
    /// Defaults to single-select mode. Use `with_selection()` for multi-select.
    pub fn new(options: impl IntoIterator<Item = (T, impl Into<String>)>) -> Self {
        Self {
            open: false,
            selection: Selection::single(),
            options: options.into_iter().map(|(v, l)| (v, l.into())).collect(),
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

    /// Set the initial selected value.
    pub fn with_value(mut self, value: T) -> Self {
        self.selection.selected.insert(value);
        self
    }

    /// Get the single selected value (for single-select mode).
    ///
    /// Returns `None` if nothing is selected or if in multi-select mode with multiple selections.
    pub fn value(&self) -> Option<&T> {
        self.selection.get_single()
    }

    /// Check if a value is selected.
    pub fn is_selected(&self, value: &T) -> bool {
        self.selection.is_selected(value)
    }

    /// Get all selected values.
    pub fn selected_values(&self) -> impl Iterator<Item = &T> {
        self.selection.get_all()
    }

    /// Get labels for all selected values.
    pub fn selected_labels(&self) -> Vec<&str> {
        self.options
            .iter()
            .filter(|(v, _)| self.selection.is_selected(v))
            .map(|(_, label)| label.as_str())
            .collect()
    }
}

/// Typestate marker: select needs a state reference.
pub struct NeedsState;

/// Typestate marker: select has a state reference.
pub struct HasState<'a, T: Clone + Eq + Hash>(&'a State<SelectState<T>>);

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
    label: Option<String>,
    disabled: bool,
    style: Option<Style>,
    style_focused: Option<Style>,
    style_disabled: Option<Style>,
    style_selected: Option<Style>,
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
            label: None,
            disabled: false,
            style: None,
            style_focused: None,
            style_disabled: None,
            style_selected: None,
            transitions: None,
        }
    }

    /// Set the state reference. Required before calling `build()`.
    pub fn state<T: Clone + Eq + Hash + PartialEq + Send + Sync + 'static>(
        self,
        s: &State<SelectState<T>>,
    ) -> Select<HasState<'_, T>> {
        Select {
            state_marker: HasState(s),
            id: self.id,
            placeholder: self.placeholder,
            label: self.label,
            disabled: self.disabled,
            style: self.style,
            style_focused: self.style_focused,
            style_disabled: self.style_disabled,
            style_selected: self.style_selected,
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

    /// Set the label text (displayed above the select).
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
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

    /// Set the style for selected items in the dropdown.
    pub fn style_selected(mut self, s: Style) -> Self {
        self.style_selected = Some(s);
        self
    }

    /// Set transitions.
    pub fn transitions(mut self, t: Transitions) -> Self {
        self.transitions = Some(t);
        self
    }
}

impl<'a, T: Clone + Eq + Hash + PartialEq + Send + Sync + 'static> Select<HasState<'a, T>> {
    /// Build the select element.
    ///
    /// Registers the toggle and option handlers if not disabled.
    pub fn build(self, registry: &HandlerRegistry, handlers: &WidgetHandlers) -> Element {
        let state = self.state_marker.0;
        let current = state.get();
        let id = self.id.clone().unwrap_or_else(|| "select".into());
        let is_multi = current.selection.mode == SelectionMode::Multi;

        log::debug!(
            "Select::build id={} open={} options_count={} multi={}",
            id,
            current.open,
            current.options.len(),
            is_multi
        );

        // Display text: selected label(s) or placeholder
        let placeholder = self
            .placeholder
            .clone()
            .unwrap_or_else(|| "Select...".into());
        let display_text = if current.selection.selected.is_empty() {
            placeholder.clone()
        } else if is_multi {
            // Show count or comma-separated labels for multi-select
            let selected_labels = current.selected_labels();
            if selected_labels.len() <= 2 {
                selected_labels.join(", ")
            } else {
                format!("{} selected", selected_labels.len())
            }
        } else {
            // Single-select: show the selected label
            current
                .value()
                .and_then(|v| current.options.iter().find(|(ov, _)| ov == v))
                .map(|(_, label)| label.clone())
                .unwrap_or_else(|| placeholder.clone())
        };

        // Calculate min width: max of all option labels and placeholder + arrow + gap
        let max_label_width = current
            .options
            .iter()
            .map(|(_, label)| label.chars().count())
            .max()
            .unwrap_or(0)
            .max(placeholder.chars().count());
        // +2 for arrow and gap
        let min_width = (max_label_width + 2) as u16;

        // Build toggle row
        let arrow = if current.open { "▲" } else { "▼" };
        let mut toggle = Element::row()
            .id(&id)
            .min_width(min_width)
            .justify(tuidom::Justify::SpaceBetween)
            .focusable(!self.disabled)
            .clickable(!self.disabled)
            .disabled(self.disabled)
            .children(vec![Element::text(&display_text), Element::text(arrow)]);

        if let Some(style) = self.style.clone() {
            toggle = toggle.style(style);
        }
        let focused_style = Style::new()
            .background(Color::var("select.focused"))
            .merge(&self.style_focused);
        let disabled_style = Style::new()
            .background(Color::var("select.disabled"))
            .merge(&self.style_disabled);
        toggle = toggle.style_focused(focused_style);
        toggle = toggle.style_disabled(disabled_style);
        if let Some(transitions) = self.transitions.clone() {
            toggle = toggle.transitions(transitions);
        }

        // Register toggle handler
        if !self.disabled {
            let state_clone = state.clone();
            let toggle_id = id.clone();
            registry.register(
                &id,
                "on_activate",
                Arc::new(move |_hx| {
                    log::debug!("[select] toggle on_activate: id={}, toggling open state", toggle_id);
                    state_clone.update(|s| s.open = !s.open);
                }),
            );

            // Register blur handler to close dropdown when focus leaves the widget
            let state_clone = state.clone();
            let base_id = id.clone();
            registry.register(
                &id,
                "on_blur",
                Arc::new(move |hx| {
                    // Only close if focus moved outside this select widget
                    let blur_target = hx.event().blur_target();
                    let should_close = match &blur_target {
                        Some(new_target) => !new_target.starts_with(&base_id),
                        None => true, // Escape or focus lost entirely
                    };
                    log::debug!(
                        "[select] on_blur: base_id={}, blur_target={:?}, should_close={}",
                        base_id, blur_target, should_close
                    );
                    if should_close {
                        state_clone.update(|s| s.open = false);
                    }
                }),
            );
        }

        // Build dropdown if open
        let elem = if current.open {
            log::debug!(
                "[select] Building dropdown id={}, options_count={}, options={:?}",
                id,
                current.options.len(),
                current.options.iter().map(|(_, l)| l.as_str()).collect::<Vec<_>>()
            );
            let dropdown_id = format!("{}-dropdown", id);
            let mut options_col = Element::col().id(&dropdown_id);

            // Get selected style (with default)
            let selected_style = self
                .style_selected
                .clone()
                .unwrap_or_else(|| Style::new().background(Color::var("select.item_selected")));

            for (i, (value, label)) in current.options.iter().enumerate() {
                let opt_id = format!("{}-opt-{}", id, i);
                let is_selected = current.selection.is_selected(value);
                log::debug!("[select] Building option id={}, label={}, selected={}", opt_id, label, is_selected);

                // Use a full-width row to ensure the entire option area is focusable/clickable
                // This prevents focus gaps that would cause focus-follows-mouse to hit elements underneath
                let mut text_elem = Element::text(label);
                if is_selected {
                    text_elem = text_elem.style(Style::new().bold());
                }

                let mut opt_elem = Element::row()
                    .id(&opt_id)
                    .width(tuidom::Size::Fill)
                    .focusable(true)
                    .clickable(true)
                    .style_focused(
                        Style::new()
                            .background(Color::var("select.item_focused"))
                            .foreground(Color::var("text.inverted")),
                    )
                    .child(text_elem);

                // Apply selected style
                if is_selected {
                    opt_elem = opt_elem.style(selected_style.clone());
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
                        let is_multi = state_clone.get().selection.mode == SelectionMode::Multi;
                        state_clone.update(|s| {
                            s.selection.toggle(value_clone.clone());
                            // In single-select mode, close dropdown
                            if !is_multi {
                                s.open = false;
                            }
                            // In multi-select mode, stay open
                        });
                        if let Some(ref handler) = on_change {
                            handler(hx);
                        }
                    }),
                );

                // Register blur handler for option
                let state_clone = state.clone();
                let base_id = id.clone();
                let opt_id_for_log = opt_id.clone();
                registry.register(
                    &opt_id,
                    "on_blur",
                    Arc::new(move |hx| {
                        let blur_target = hx.event().blur_target();
                        let should_close = match &blur_target {
                            Some(new_target) => !new_target.starts_with(&base_id),
                            None => true,
                        };
                        log::debug!(
                            "[select] option on_blur: opt_id={}, base_id={}, blur_target={:?}, should_close={}",
                            opt_id_for_log, base_id, blur_target, should_close
                        );
                        if should_close {
                            state_clone.update(|s| s.open = false);
                        }
                    }),
                );
            }

            // Use absolute positioning for dropdown overlay
            use tuidom::{Overflow, Position, Size};

            // Calculate dropdown height - cap at 10 items to enable scrolling
            let dropdown_height = (current.options.len() as u16).min(10);

            let dropdown = options_col
                .position(Position::Absolute)
                .top(1) // Below the toggle
                .left(-1) // Extend background left for visual separation
                .padding(tuidom::Edges::left(1)) // Keep content aligned with toggle
                .width(Size::Fixed(min_width + 1)) // Account for extra padding
                .height(Size::Fixed(dropdown_height)) // Cap height for scrolling
                .overflow(Overflow::Auto) // Enable scrolling when content overflows
                .z_index(1)
                .interaction_scope(true) // Scope focus/clicks to dropdown
                .style(Style::new().background(Color::var("select.dropdown_bg")));

            // Register on_scope_click handler to close dropdown when clicking backdrop
            let state_clone = state.clone();
            let dropdown_id_for_log = dropdown_id.clone();
            registry.register(
                &dropdown_id,
                "on_scope_click",
                Arc::new(move |_hx| {
                    log::debug!("[select] on_scope_click fired for {}", dropdown_id_for_log);
                    state_clone.update(|s| s.open = false);
                }),
            );

            Element::box_()
                .width(Size::Fixed(min_width))
                .height(Size::Fixed(1)) // Only take toggle's height
                .child(toggle)
                .child(dropdown)
        } else {
            toggle.width(tuidom::Size::Fixed(min_width))
        };

        // Wrap in column with label if label is present
        if let Some(label) = &self.label {
            Element::col()
                .child(
                    Element::text(label)
                        .style(Style::new().foreground(Color::var("text.muted"))),
                )
                .child(elem)
        } else {
            elem
        }
    }
}
