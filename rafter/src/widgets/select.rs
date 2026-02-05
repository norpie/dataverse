//! Select widget - a dropdown selection component.

use std::hash::Hash;
use std::sync::Arc;

use tuidom::{Color, Element, Overflow, Position, Size, Style, Transitions};

use super::scroll::{ScrollState, ScrollableWidgetState, Scrollbar};
use super::selection::{Selection, SelectionMode};
use super::virtual_scroller::VirtualScroller;
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
    /// Scroll state for virtualized dropdown.
    pub scroll: ScrollState,
    /// Virtual scroller for dropdown virtualization.
    pub(crate) scroller: VirtualScroller,
    /// Scrollbar rect for drag calculations.
    scrollbar_rect: Option<(u16, u16, u16, u16)>,
    /// Drag grab offset for scrollbar.
    drag_grab_offset: Option<u16>,
}

impl<T: Clone + Eq + Hash> Default for SelectState<T> {
    fn default() -> Self {
        Self {
            open: false,
            selection: Selection::single(),
            options: Vec::new(),
            scroll: ScrollState::new(),
            scroller: VirtualScroller::new(),
            scrollbar_rect: None,
            drag_grab_offset: None,
        }
    }
}

impl<T: Clone + Eq + Hash> SelectState<T> {
    /// Create a new SelectState with the given options.
    ///
    /// Defaults to single-select mode. Use `with_selection()` for multi-select.
    pub fn new(options: impl IntoIterator<Item = (T, impl Into<String>)>) -> Self {
        let mut state = Self::default();
        state.set_options(options);
        state
    }

    /// Set options and rebuild scroller.
    pub fn set_options(&mut self, options: impl IntoIterator<Item = (T, impl Into<String>)>) {
        self.options = options.into_iter().map(|(v, l)| (v, l.into())).collect();
        // Each option is 1 row high
        let total = self
            .scroller
            .rebuild(std::iter::repeat_n(1, self.options.len()));
        self.scroll.set_content_height(total);
    }

    /// Set the selection mode.
    pub fn with_selection(mut self, mode: SelectionMode) -> Self {
        self.selection = match mode {
            SelectionMode::None => Selection::none(),
            SelectionMode::Single => Selection::single(),
            SelectionMode::Forced => Selection::forced(),
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

// Implement ScrollableWidgetState for SelectState (for scrollbar support)
impl<T: Clone + Eq + Hash + Send + Sync + 'static> ScrollableWidgetState for SelectState<T> {
    fn scroll(&self) -> &ScrollState {
        &self.scroll
    }

    fn scroll_mut(&mut self) -> &mut ScrollState {
        &mut self.scroll
    }

    fn scrollbar_rect(&self) -> Option<(u16, u16, u16, u16)> {
        self.scrollbar_rect
    }

    fn set_scrollbar_rect(&mut self, rect: Option<(u16, u16, u16, u16)>) {
        self.scrollbar_rect = rect;
    }

    fn drag_grab_offset(&self) -> Option<u16> {
        self.drag_grab_offset
    }

    fn set_drag_grab_offset(&mut self, offset: Option<u16>) {
        self.drag_grab_offset = offset;
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
    min_width: Option<u16>,
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
            min_width: None,
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
            min_width: self.min_width,
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
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Set the select toggle width in characters.
    /// This controls the width of the toggle button and dropdown.
    pub fn toggle_width(mut self, width: u16) -> Self {
        self.min_width = Some(width);
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

        // Calculate width: use explicit min_width if provided, otherwise calculate from options
        let min_width = if let Some(w) = self.min_width {
            w
        } else {
            let max_label_width = current
                .options
                .iter()
                .map(|(_, label)| label.chars().count())
                .max()
                .unwrap_or(0)
                .max(placeholder.chars().count());
            // +2 for arrow and gap
            (max_label_width + 2) as u16
        };

        // Build toggle row
        let arrow = if current.open { "▲" } else { "▼" };

        log::debug!(
            "[select] id={} open={} min_width={} explicit_min_width={:?} display_text_len={}",
            id,
            current.open,
            min_width,
            self.min_width,
            display_text.chars().count()
        );

        // Build toggle content (text + arrow)
        let toggle_content = Element::row()
            .justify(tuidom::Justify::SpaceBetween)
            .width(Size::Fill)
            .children(vec![Element::text(&display_text), Element::text(arrow)]);

        let mut toggle = Element::box_()
            .id(&id)
            .width(tuidom::Size::Fixed(min_width))
            .flex_shrink(0)
            .padding(tuidom::Edges::symmetric(0, 1))
            .focusable(!self.disabled)
            .clickable(!self.disabled)
            .disabled(self.disabled)
            .child(toggle_content);

        let style = Style::new()
            .background(Color::var("button.normal"))
            .merge(&self.style);
        let focused_style = Style::new()
            .background(Color::var("button.hover"))
            .merge(&self.style_focused);
        let disabled_style = Style::new()
            .background(Color::var("button.disabled"))
            .merge(&self.style_disabled);
        toggle = toggle.style(style);
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
                    log::debug!(
                        "[select] toggle on_activate: id={}, toggling open state",
                        toggle_id
                    );
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
                    // Close if focus moved outside this widget or no new target (e.g. Escape)
                    let blur_target = hx.event().blur_target();
                    let should_close = match &blur_target {
                        Some(new_target) => !new_target.starts_with(&base_id),
                        None => true,
                    };
                    log::debug!(
                        "[select] on_blur: base_id={}, blur_target={:?}, should_close={}",
                        base_id,
                        blur_target,
                        should_close
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
                "[select] Building dropdown id={}, options_count={}",
                id,
                current.options.len()
            );
            let dropdown_id = format!("{}-dropdown", id);
            let body_id = format!("{}-body", id);

            // Get selected style (with default)
            let selected_style = self
                .style_selected
                .clone()
                .unwrap_or_else(|| Style::new().background(Color::var("list.item_selected")));

            // Calculate dropdown height - cap at 10 items
            let max_visible = 10u16;
            let dropdown_height = (current.options.len() as u16).min(max_visible);

            // Set viewport for virtualization
            state.update(|s| {
                s.scroll.set_viewport(dropdown_height);
            });

            // Get visible range using virtualization
            let visible_range = current.scroller.visible_range(&current.scroll);
            let visible_count = visible_range.len();
            let total_options = current.options.len();

            let mut options_col = Element::col()
                .id(&body_id)
                .width(Size::Fill)
                .height(Size::Fill)
                .overflow(Overflow::Hidden)
                .scrollable(true);

            for (pos_in_visible, i) in visible_range.enumerate() {
                if let Some((value, label)) = current.options.get(i) {
                    let opt_id = format!("{}-opt-{}", id, i);
                    let is_selected = current.selection.is_selected(value);

                    let mut text_elem = Element::text(label);
                    if is_selected {
                        text_elem = text_elem.style(Style::new().bold());
                    }

                    let mut opt_elem = Element::row()
                        .id(&opt_id)
                        .width(Size::Fill)
                        .height(Size::Fixed(1))
                        .focusable(true)
                        .clickable(true)
                        .style_focused(
                            Style::new()
                                .background(Color::var("list.item_focused"))
                                .foreground(Color::var("text.inverted")),
                        )
                        .child(text_elem);

                    if is_selected {
                        opt_elem = opt_elem.style(selected_style.clone());
                    }

                    options_col = options_col.child(opt_elem);

                    // Register option activate handler
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
                                if !is_multi {
                                    s.open = false;
                                }
                            });
                            if let Some(ref handler) = on_change {
                                handler(hx);
                            }
                        }),
                    );

                    // Register blur handler
                    let state_clone = state.clone();
                    let base_id = id.clone();
                    registry.register(
                        &opt_id,
                        "on_blur",
                        Arc::new(move |hx| {
                            // Close if focus moved outside this widget or no new target (e.g. Escape)
                            let blur_target = hx.event().blur_target();
                            let should_close = match &blur_target {
                                Some(new_target) => !new_target.starts_with(&base_id),
                                None => true,
                            };
                            if should_close {
                                state_clone.update(|s| s.open = false);
                            }
                        }),
                    );

                    // Register boundary scroll handlers
                    let is_at_top = pos_in_visible == 0 && i > 0;
                    let is_at_bottom = pos_in_visible == visible_count - 1 && i < total_options - 1;

                    if is_at_top {
                        let state_clone = state.clone();
                        let id_clone = id.clone();
                        let target_index = i.saturating_sub(1);
                        registry.register(
                            &opt_id,
                            "on_key_up",
                            Arc::new(move |hx| {
                                state_clone.update(|s| {
                                    s.scroll
                                        .apply_request(super::scroll::ScrollRequest::Delta(-1));
                                });
                                let target_id = format!("{}-opt-{}", id_clone, target_index);
                                hx.cx().focus(&target_id);
                            }),
                        );
                    }

                    if is_at_bottom {
                        let state_clone = state.clone();
                        let id_clone = id.clone();
                        let target_index = i + 1;
                        registry.register(
                            &opt_id,
                            "on_key_down",
                            Arc::new(move |hx| {
                                state_clone.update(|s| {
                                    s.scroll
                                        .apply_request(super::scroll::ScrollRequest::Delta(1));
                                });
                                let target_id = format!("{}-opt-{}", id_clone, target_index);
                                hx.cx().focus(&target_id);
                            }),
                        );
                    }
                }
            }

            // Register scroll handler for mouse wheel and keyboard on body
            {
                let state_clone = state.clone();
                let id_clone = id.clone();
                registry.register(
                    &body_id,
                    "on_scroll",
                    Arc::new(move |hx| {
                        // Mouse wheel
                        if let Some((_, delta_y)) = hx.event().scroll_delta() {
                            state_clone.update(|s| {
                                s.scroll
                                    .apply_request(super::scroll::ScrollRequest::Delta(delta_y));
                            });
                        }
                        // Keyboard scroll actions (PageUp/Down/Home/End)
                        if let Some(action) = hx.event().scroll_action() {
                            use super::scroll::ScrollRequest;
                            use tuidom::ScrollAction;

                            state_clone.update(|s| {
                                let request = match action {
                                    ScrollAction::PageUp => ScrollRequest::PageUp,
                                    ScrollAction::PageDown => ScrollRequest::PageDown,
                                    ScrollAction::Home => ScrollRequest::Home,
                                    ScrollAction::End => ScrollRequest::End,
                                };
                                s.scroll.apply_request(request);
                            });

                            // Focus the appropriate item after scroll
                            let current = state_clone.get();
                            let total = current.options.len();
                            if total == 0 {
                                return;
                            }

                            let target_index = match action {
                                ScrollAction::Home => 0,
                                ScrollAction::End => total - 1,
                                ScrollAction::PageUp | ScrollAction::PageDown => {
                                    current.scroller.first_visible_index(&current.scroll)
                                }
                            };

                            let target_id = format!("{}-opt-{}", id_clone, target_index);
                            hx.cx().focus(&target_id);
                        }
                    }),
                );
            }

            // Build dropdown with scrollbar if needed
            let show_scrollbar = current.options.len() > max_visible as usize;

            let dropdown_content = if show_scrollbar {
                let scrollbar_id = format!("{}-scrollbar", id);
                let scrollbar = Scrollbar::vertical()
                    .id(&scrollbar_id)
                    .state(state)
                    .build(registry, handlers);

                Element::row()
                    .width(Size::Fixed(min_width + 1))
                    .height(Size::Fixed(dropdown_height))
                    .child(options_col)
                    .child(scrollbar)
            } else {
                options_col
                    .width(Size::Fixed(min_width + 1))
                    .height(Size::Fixed(dropdown_height))
            };

            let dropdown = dropdown_content
                .id(&dropdown_id)
                .position(Position::Absolute)
                .top(1)
                .left(-1)
                .padding(tuidom::Edges::left(1))
                .z_index(2)
                .interaction_scope(true)
                .style(Style::new().background(Color::var("select.dropdown_bg")));

            // Full-screen backdrop to capture clicks outside dropdown
            let backdrop_id = format!("{}-backdrop", id);
            let backdrop = Element::box_()
                .id(&backdrop_id)
                .position(Position::Absolute)
                .left(0)
                .top(0)
                .width(Size::Percent(1.0))
                .height(Size::Percent(1.0))
                .z_index(1)
                .clickable(true);

            // Register on_activate on backdrop to close dropdown
            let state_clone = state.clone();
            registry.register(
                &backdrop_id,
                "on_activate",
                Arc::new(move |_hx| {
                    state_clone.update(|s| s.open = false);
                }),
            );

            // Wrap toggle in a positioned container with backdrop and dropdown
            Element::box_()
                .width(Size::Fixed(min_width))
                .height(Size::Fixed(1))
                .flex_shrink(0)
                .child(toggle)
                .child(backdrop)
                .child(dropdown)
        } else {
            // Reset scroll when closed
            state.update(|s| s.scroll.offset = 0);
            toggle
        };

        // Wrap in column with label if label is present
        if let Some(label) = &self.label {
            Element::col()
                .child(
                    Element::text(label).style(Style::new().foreground(Color::var("text.muted"))),
                )
                .child(elem)
        } else {
            elem
        }
    }
}
