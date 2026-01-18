//! Autocomplete widget - a text input with fuzzy-filtered dropdown suggestions.

use std::hash::Hash;
use std::sync::Arc;

use nucleo_matcher::pattern::{AtomKind, CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};
use tuidom::{Color, Element, Overflow, Position, Size, Style, Transitions};

use super::scroll::{ScrollState, ScrollableWidgetState, Scrollbar};
use super::selection::{Selection, SelectionMode};
use super::virtual_scroller::VirtualScroller;
use crate::state::State;
use crate::{HandlerRegistry, WidgetHandlers};

/// Result of a fuzzy filter operation.
#[derive(Debug, Clone)]
pub struct FilterMatch {
    /// Index of the matched item in the original list.
    pub index: usize,
    /// Match score (higher is better).
    pub score: u32,
}

/// Fuzzy filter using nucleo-matcher.
///
/// Returns matches sorted by score (highest first).
/// Empty query returns all items with score 0.
fn fuzzy_filter(query: &str, labels: &[String]) -> Vec<FilterMatch> {
    if query.is_empty() {
        return labels
            .iter()
            .enumerate()
            .map(|(index, _)| FilterMatch { index, score: 0 })
            .collect();
    }

    let mut matcher = Matcher::new(Config::DEFAULT);
    let pattern = Pattern::new(
        query,
        CaseMatching::Ignore,
        Normalization::Smart,
        AtomKind::Fuzzy,
    );

    let mut matches: Vec<FilterMatch> = labels
        .iter()
        .enumerate()
        .filter_map(|(index, label)| {
            let mut buf = Vec::new();
            let haystack = Utf32Str::new(label, &mut buf);
            pattern
                .score(haystack, &mut matcher)
                .map(|score| FilterMatch { index, score })
        })
        .collect();

    // Sort by score descending (higher score = better match)
    matches.sort_by(|a, b| b.score.cmp(&a.score));

    matches
}

/// State for an autocomplete widget.
///
/// Contains the dropdown state, input text, selected value(s), and available options.
///
/// # Example
///
/// ```ignore
/// // In app struct (will be wrapped in State<> by #[app] macro):
/// country: AutocompleteState<String>,
///
/// // Initialize in on_start (single-select, the default):
/// self.country.set(AutocompleteState::new([
///     ("us".to_string(), "United States"),
///     ("uk".to_string(), "United Kingdom"),
///     ("de".to_string(), "Germany"),
/// ]));
///
/// // Multi-select mode:
/// self.tags.set(AutocompleteState::new([...]).with_selection(SelectionMode::Multi));
/// ```
#[derive(Clone, Debug)]
pub struct AutocompleteState<T: Clone + Eq + Hash> {
    /// Whether the dropdown is open.
    pub open: bool,
    /// Current input text.
    pub text: String,
    /// Selection state (supports single and multi-select).
    pub selection: Selection<T>,
    /// Dropdown cursor position (index into filtered).
    pub cursor: usize,
    /// Available options as (value, label) pairs.
    pub options: Vec<(T, String)>,
    /// Cached filtered indices (indices into options), sorted by score.
    pub filtered: Vec<FilterMatch>,
    /// Scroll state for virtualized dropdown.
    pub scroll: ScrollState,
    /// Virtual scroller for dropdown virtualization.
    pub(crate) scroller: VirtualScroller,
    /// Scrollbar rect for drag calculations.
    scrollbar_rect: Option<(u16, u16, u16, u16)>,
    /// Drag grab offset for scrollbar.
    drag_grab_offset: Option<u16>,
}

impl<T: Clone + Eq + Hash> Default for AutocompleteState<T> {
    fn default() -> Self {
        Self {
            open: false,
            text: String::new(),
            selection: Selection::single(),
            cursor: 0,
            options: Vec::new(),
            filtered: Vec::new(),
            scroll: ScrollState::new(),
            scroller: VirtualScroller::new(),
            scrollbar_rect: None,
            drag_grab_offset: None,
        }
    }
}

impl<T: Clone + Eq + Hash> AutocompleteState<T> {
    /// Create a new AutocompleteState with the given options.
    ///
    /// Defaults to single-select mode. Use `with_selection()` for multi-select.
    pub fn new(options: impl IntoIterator<Item = (T, impl Into<String>)>) -> Self {
        let options: Vec<(T, String)> = options
            .into_iter()
            .map(|(v, l)| (v, l.into()))
            .collect();
        let labels: Vec<String> = options.iter().map(|(_, l)| l.clone()).collect();
        let filtered = fuzzy_filter("", &labels);

        // Build scroller for filtered items
        let mut scroller = VirtualScroller::new();
        let total = scroller.rebuild(std::iter::repeat(1).take(filtered.len()));

        let mut scroll = ScrollState::new();
        scroll.set_content_height(total);

        Self {
            open: false,
            text: String::new(),
            selection: Selection::single(),
            cursor: 0,
            options,
            filtered,
            scroll,
            scroller,
            scrollbar_rect: None,
            drag_grab_offset: None,
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

    /// Set the initial selected value (also sets text to matching label in single-select mode).
    pub fn with_value(mut self, value: T) -> Self
    where
        T: PartialEq,
    {
        if self.selection.mode == SelectionMode::Single {
            if let Some((_, label)) = self.options.iter().find(|(v, _)| v == &value) {
                self.text = label.clone();
            }
        }
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

    /// Re-run the fuzzy filter with current text.
    pub fn refilter(&mut self) {
        let labels: Vec<String> = self.options.iter().map(|(_, l)| l.clone()).collect();
        self.filtered = fuzzy_filter(&self.text, &labels);
        // Reset cursor if out of bounds
        if self.cursor >= self.filtered.len() {
            self.cursor = 0;
        }
        // Rebuild scroller for new filtered items
        let total = self.scroller.rebuild(std::iter::repeat(1).take(self.filtered.len()));
        self.scroll.set_content_height(total);
        self.scroll.offset = 0; // Reset scroll on filter change
    }

    /// Get the label at a filtered index.
    pub fn filtered_label(&self, filtered_index: usize) -> Option<&str> {
        self.filtered
            .get(filtered_index)
            .and_then(|m| self.options.get(m.index))
            .map(|(_, label)| label.as_str())
    }

    /// Get the value at a filtered index.
    pub fn filtered_value(&self, filtered_index: usize) -> Option<&T> {
        self.filtered
            .get(filtered_index)
            .and_then(|m| self.options.get(m.index))
            .map(|(value, _)| value)
    }
}

// Implement ScrollableWidgetState for AutocompleteState (for scrollbar support)
impl<T: Clone + Eq + Hash + Send + Sync + 'static> ScrollableWidgetState for AutocompleteState<T> {
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

/// Typestate marker: autocomplete needs a state reference.
pub struct NeedsState;

/// Typestate marker: autocomplete has a state reference.
pub struct HasState<'a, T: Clone + Eq + Hash>(&'a State<AutocompleteState<T>>);

/// An autocomplete widget builder.
///
/// Uses typestate pattern to enforce `state()` is called before `build()`.
///
/// # Example
///
/// ```ignore
/// // In page! macro:
/// autocomplete (state: self.country, id: "country", placeholder: "Search countries...")
///     style (bg: surface)
///     on_select: country_selected()
/// ```
#[derive(Clone, Debug)]
pub struct Autocomplete<S = NeedsState> {
    state_marker: S,
    id: Option<String>,
    placeholder: Option<String>,
    label: Option<String>,
    disabled: bool,
    width: Option<u16>,
    style: Option<Style>,
    style_focused: Option<Style>,
    style_disabled: Option<Style>,
    style_selected: Option<Style>,
    transitions: Option<Transitions>,
}

impl Default for Autocomplete<NeedsState> {
    fn default() -> Self {
        Self::new()
    }
}

impl Autocomplete<NeedsState> {
    /// Create a new autocomplete builder.
    pub fn new() -> Self {
        Self {
            state_marker: NeedsState,
            id: None,
            placeholder: None,
            label: None,
            disabled: false,
            width: None,
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
        s: &State<AutocompleteState<T>>,
    ) -> Autocomplete<HasState<'_, T>> {
        Autocomplete {
            state_marker: HasState(s),
            id: self.id,
            placeholder: self.placeholder,
            label: self.label,
            disabled: self.disabled,
            width: self.width,
            style: self.style,
            style_focused: self.style_focused,
            style_disabled: self.style_disabled,
            style_selected: self.style_selected,
            transitions: self.transitions,
        }
    }
}

impl<S> Autocomplete<S> {
    /// Set the autocomplete id.
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Set the placeholder text shown when input is empty.
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    /// Set the label text (displayed above the autocomplete).
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Mark the autocomplete as disabled.
    pub fn disabled(mut self) -> Self {
        self.disabled = true;
        self
    }

    /// Set the input width in characters.
    pub fn width(mut self, width: u16) -> Self {
        self.width = Some(width);
        self
    }

    /// Set the autocomplete style.
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

impl<'a, T: Clone + Eq + Hash + PartialEq + Send + Sync + 'static> Autocomplete<HasState<'a, T>> {
    /// Build the autocomplete element.
    ///
    /// Registers handlers for text input, option selection, and blur.
    pub fn build(self, registry: &HandlerRegistry, handlers: &WidgetHandlers) -> Element {
        let state = self.state_marker.0;
        let current = state.get();
        let id = self.id.clone().unwrap_or_else(|| "autocomplete".into());
        let is_multi = current.selection.mode == SelectionMode::Multi;

        log::debug!(
            "Autocomplete::build id={} open={} text={} options_count={} filtered_count={} multi={}",
            id,
            current.open,
            current.text,
            current.options.len(),
            current.filtered.len(),
            is_multi
        );

        // Calculate width
        let placeholder = self.placeholder.clone().unwrap_or_default();
        let max_label_width = current
            .options
            .iter()
            .map(|(_, label)| label.chars().count())
            .max()
            .unwrap_or(0)
            .max(placeholder.chars().count());
        let min_width = self.width.unwrap_or((max_label_width + 2) as u16);

        // Build text input element
        let mut input = Element::text_input(&current.text)
            .id(&id)
            .focusable(!self.disabled)
            .captures_input(!self.disabled)
            .clickable(!self.disabled)
            .disabled(self.disabled)
            .width(Size::Fixed(min_width));

        if !placeholder.is_empty() {
            input = input.placeholder(&placeholder);
        }

        if let Some(style) = self.style.clone() {
            input = input.style(style);
        }
        let focused_style = Style::new()
            .background(Color::var("autocomplete.focused"))
            .merge(&self.style_focused);
        let disabled_style = Style::new()
            .background(Color::var("autocomplete.disabled"))
            .merge(&self.style_disabled);
        input = input.style_focused(focused_style);
        input = input.style_disabled(disabled_style);
        if let Some(transitions) = self.transitions.clone() {
            input = input.transitions(transitions);
        }

        // Register handlers if not disabled
        if !self.disabled {
            // on_change: update text, refilter, open dropdown
            let state_clone = state.clone();
            let user_on_change = handlers.get("on_change").cloned();
            registry.register(
                &id,
                "on_change",
                Arc::new(move |hx| {
                    if let Some(text) = hx.event().text() {
                        state_clone.update(|s| {
                            s.text = text.to_string();
                            s.refilter();
                            s.open = true;
                            s.cursor = 0;
                        });
                    }
                    if let Some(ref handler) = user_on_change {
                        handler(hx);
                    }
                }),
            );

            // on_blur: close dropdown when focus leaves the widget
            let state_clone = state.clone();
            let base_id = id.clone();
            registry.register(
                &id,
                "on_blur",
                Arc::new(move |hx| {
                    // Close if focus moved outside this widget or no new target (e.g. Escape)
                    let should_close = match hx.event().blur_target() {
                        Some(new_target) => !new_target.starts_with(&base_id),
                        None => true,
                    };
                    if should_close {
                        state_clone.update(|s| s.open = false);
                    }
                }),
            );

            // on_activate: open dropdown when clicked
            let state_clone = state.clone();
            registry.register(
                &id,
                "on_activate",
                Arc::new(move |_hx| {
                    state_clone.update(|s| s.open = true);
                }),
            );

            // on_submit: open dropdown if closed, select item at cursor if open
            let state_clone = state.clone();
            let on_select = handlers.get("on_select").cloned();
            registry.register(
                &id,
                "on_submit",
                Arc::new(move |hx| {
                    let current = state_clone.get();
                    if !current.open {
                        // Open dropdown on activate
                        state_clone.update(|s| s.open = true);
                    } else if !current.filtered.is_empty() {
                        // Select item at cursor
                        let cursor = current.cursor;
                        let is_multi = current.selection.mode == SelectionMode::Multi;
                        if let Some(filter_match) = current.filtered.get(cursor) {
                            if let Some((value, label)) = current.options.get(filter_match.index) {
                                let value = value.clone();
                                let label = label.clone();
                                state_clone.update(|s| {
                                    s.selection.toggle(value);
                                    // In single-select mode, update text and close dropdown
                                    if !is_multi {
                                        s.text = label;
                                        s.open = false;
                                        s.refilter();
                                    }
                                    // In multi-select mode, stay open
                                });
                                if let Some(ref handler) = on_select {
                                    handler(hx);
                                }
                            }
                        }
                    }
                }),
            );
        }

        // Build dropdown if open
        let elem = if current.open && !current.filtered.is_empty() {
            let dropdown_id = format!("{}-dropdown", id);
            let body_id = format!("{}-body", id);

            // Get selected style (with default)
            let selected_style = self
                .style_selected
                .clone()
                .unwrap_or_else(|| Style::new().background(Color::var("list.item_selected")));

            // Calculate dropdown height - cap at 10 items
            let max_visible = 10u16;
            let dropdown_height = (current.filtered.len() as u16).min(max_visible);

            // Set viewport for virtualization
            state.update(|s| {
                s.scroll.set_viewport(dropdown_height);
            });

            // Get visible range using virtualization
            let visible_range = current.scroller.visible_range(&current.scroll);
            let visible_count = visible_range.len();
            let total_filtered = current.filtered.len();

            let mut options_col = Element::col()
                .id(&body_id)
                .width(Size::Fill)
                .height(Size::Fill)
                .overflow(Overflow::Hidden)
                .scrollable(true);

            for (pos_in_visible, i) in visible_range.enumerate() {
                if let Some(filter_match) = current.filtered.get(i) {
                    if let Some((value, label)) = current.options.get(filter_match.index) {
                        let opt_id = format!("{}-opt-{}", id, i);
                        let is_cursor = i == current.cursor;
                        let is_selected = current.selection.is_selected(value);

                        let mut text_elem = Element::text(label);
                        if is_cursor {
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
                        let label_clone = label.clone();
                        let on_select = handlers.get("on_select").cloned();
                        registry.register(
                            &opt_id,
                            "on_activate",
                            Arc::new(move |hx| {
                                let is_multi = state_clone.get().selection.mode == SelectionMode::Multi;
                                state_clone.update(|s| {
                                    s.selection.toggle(value_clone.clone());
                                    if !is_multi {
                                        s.text = label_clone.clone();
                                        s.open = false;
                                        s.refilter();
                                    }
                                });
                                if let Some(ref handler) = on_select {
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
                                let should_close = match hx.event().blur_target() {
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
                        let is_at_bottom = pos_in_visible == visible_count - 1 && i < total_filtered - 1;

                        if is_at_top {
                            let state_clone = state.clone();
                            let id_clone = id.clone();
                            let target_index = i.saturating_sub(1);
                            registry.register(
                                &opt_id,
                                "on_key_up",
                                Arc::new(move |hx| {
                                    state_clone.update(|s| {
                                        s.scroll.apply_request(super::scroll::ScrollRequest::Delta(-1));
                                        s.cursor = target_index;
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
                                        s.scroll.apply_request(super::scroll::ScrollRequest::Delta(1));
                                        s.cursor = target_index;
                                    });
                                    let target_id = format!("{}-opt-{}", id_clone, target_index);
                                    hx.cx().focus(&target_id);
                                }),
                            );
                        }
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
                                s.scroll.apply_request(super::scroll::ScrollRequest::Delta(delta_y));
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
                            let total = current.filtered.len();
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
            let show_scrollbar = current.filtered.len() > max_visible as usize;

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
                .style(Style::new().background(Color::var("autocomplete.dropdown_bg")));

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

            Element::box_()
                .width(Size::Fixed(min_width))
                .height(Size::Fixed(1))
                .child(input)
                .child(backdrop)
                .child(dropdown)
        } else {
            // Reset scroll when closed
            state.update(|s| s.scroll.offset = 0);
            input
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
