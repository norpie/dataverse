//! Autocomplete widget - a text input with fuzzy-filtered dropdown suggestions.

use std::sync::Arc;

use nucleo_matcher::pattern::{AtomKind, CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};
use tuidom::{Color, Element, Overflow, Position, Size, Style, Transitions};

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
/// Contains the dropdown state, input text, selected value, and available options.
///
/// # Example
///
/// ```ignore
/// // In app struct (will be wrapped in State<> by #[app] macro):
/// country: AutocompleteState<String>,
///
/// // Initialize in on_start:
/// self.country.set(AutocompleteState::new([
///     ("us".to_string(), "United States"),
///     ("uk".to_string(), "United Kingdom"),
///     ("de".to_string(), "Germany"),
/// ]));
/// ```
#[derive(Clone, Debug)]
pub struct AutocompleteState<T: Clone> {
    /// Whether the dropdown is open.
    pub open: bool,
    /// Current input text.
    pub text: String,
    /// The currently selected value, if any.
    pub value: Option<T>,
    /// Dropdown cursor position (index into filtered).
    pub cursor: usize,
    /// Available options as (value, label) pairs.
    pub options: Vec<(T, String)>,
    /// Cached filtered indices (indices into options), sorted by score.
    pub filtered: Vec<FilterMatch>,
}

impl<T: Clone> Default for AutocompleteState<T> {
    fn default() -> Self {
        Self {
            open: false,
            text: String::new(),
            value: None,
            cursor: 0,
            options: Vec::new(),
            filtered: Vec::new(),
        }
    }
}

impl<T: Clone> AutocompleteState<T> {
    /// Create a new AutocompleteState with the given options.
    pub fn new(options: impl IntoIterator<Item = (T, impl Into<String>)>) -> Self {
        let options: Vec<(T, String)> = options
            .into_iter()
            .map(|(v, l)| (v, l.into()))
            .collect();
        let labels: Vec<String> = options.iter().map(|(_, l)| l.clone()).collect();
        let filtered = fuzzy_filter("", &labels);

        Self {
            open: false,
            text: String::new(),
            value: None,
            cursor: 0,
            options,
            filtered,
        }
    }

    /// Set the initial selected value (also sets text to matching label).
    pub fn with_value(mut self, value: T) -> Self
    where
        T: PartialEq,
    {
        if let Some((_, label)) = self.options.iter().find(|(v, _)| v == &value) {
            self.text = label.clone();
        }
        self.value = Some(value);
        self
    }

    /// Re-run the fuzzy filter with current text.
    pub fn refilter(&mut self) {
        let labels: Vec<String> = self.options.iter().map(|(_, l)| l.clone()).collect();
        self.filtered = fuzzy_filter(&self.text, &labels);
        // Reset cursor if out of bounds
        if self.cursor >= self.filtered.len() {
            self.cursor = 0;
        }
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

/// Typestate marker: autocomplete needs a state reference.
pub struct NeedsState;

/// Typestate marker: autocomplete has a state reference.
pub struct HasState<'a, T: Clone>(&'a State<AutocompleteState<T>>);

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
    disabled: bool,
    width: Option<u16>,
    style: Option<Style>,
    style_focused: Option<Style>,
    style_disabled: Option<Style>,
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
            disabled: false,
            width: None,
            style: None,
            style_focused: None,
            style_disabled: None,
            transitions: None,
        }
    }

    /// Set the state reference. Required before calling `build()`.
    pub fn state<T: Clone + PartialEq + Send + Sync + 'static>(
        self,
        s: &State<AutocompleteState<T>>,
    ) -> Autocomplete<HasState<'_, T>> {
        Autocomplete {
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

    /// Set transitions.
    pub fn transitions(mut self, t: Transitions) -> Self {
        self.transitions = Some(t);
        self
    }
}

impl<'a, T: Clone + PartialEq + Send + Sync + 'static> Autocomplete<HasState<'a, T>> {
    /// Build the autocomplete element.
    ///
    /// Registers handlers for text input, option selection, and blur.
    pub fn build(self, registry: &HandlerRegistry, handlers: &WidgetHandlers) -> Element {
        let state = self.state_marker.0;
        let current = state.get();
        let id = self.id.clone().unwrap_or_else(|| "autocomplete".into());

        log::debug!(
            "Autocomplete::build id={} open={} text={} options_count={} filtered_count={}",
            id,
            current.open,
            current.text,
            current.options.len(),
            current.filtered.len()
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
        let focused_style = self
            .style_focused
            .clone()
            .unwrap_or_else(|| Style::new().background(Color::var("autocomplete.focused")));
        let disabled_style = self
            .style_disabled
            .clone()
            .unwrap_or_else(|| Style::new().background(Color::var("autocomplete.disabled")));
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
                    if let Some(text) = hx.changed_text() {
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

            // on_focus: open dropdown
            let state_clone = state.clone();
            registry.register(
                &id,
                "on_focus",
                Arc::new(move |_hx| {
                    state_clone.update(|s| {
                        s.open = true;
                    });
                }),
            );

            // on_blur: close dropdown when focus leaves the widget
            let state_clone = state.clone();
            let base_id = id.clone();
            registry.register(
                &id,
                "on_blur",
                Arc::new(move |hx| {
                    let should_close = match hx.blur_new_target() {
                        Some(new_target) => !new_target.starts_with(&base_id),
                        None => true,
                    };
                    if should_close {
                        state_clone.update(|s| s.open = false);
                    }
                }),
            );

            // on_submit: select item at cursor if dropdown open
            let state_clone = state.clone();
            let on_select = handlers.get("on_select").cloned();
            registry.register(
                &id,
                "on_submit",
                Arc::new(move |hx| {
                    let current = state_clone.get();
                    if current.open && !current.filtered.is_empty() {
                        let cursor = current.cursor;
                        if let Some(filter_match) = current.filtered.get(cursor) {
                            if let Some((value, label)) = current.options.get(filter_match.index) {
                                let value = value.clone();
                                let label = label.clone();
                                state_clone.update(|s| {
                                    s.value = Some(value);
                                    s.text = label;
                                    s.open = false;
                                    s.refilter();
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
        if current.open && !current.filtered.is_empty() {
            let dropdown_id = format!("{}-dropdown", id);
            let mut options_col = Element::col().id(&dropdown_id);

            for (i, filter_match) in current.filtered.iter().enumerate() {
                if let Some((value, label)) = current.options.get(filter_match.index) {
                    let opt_id = format!("{}-opt-{}", id, i);
                    let is_cursor = i == current.cursor;

                    let mut text_elem = Element::text(label);
                    if is_cursor {
                        text_elem = text_elem.style(Style::new().bold());
                    }

                    let opt_elem = Element::row()
                        .id(&opt_id)
                        .width(Size::Fill)
                        .focusable(true)
                        .clickable(true)
                        .style_focused(Style::new().background(Color::var("autocomplete.item_focused")))
                        .child(text_elem);

                    options_col = options_col.child(opt_elem);

                    // Register option handler
                    let state_clone = state.clone();
                    let value_clone = value.clone();
                    let label_clone = label.clone();
                    let on_select = handlers.get("on_select").cloned();
                    registry.register(
                        &opt_id,
                        "on_activate",
                        Arc::new(move |hx| {
                            state_clone.update(|s| {
                                s.value = Some(value_clone.clone());
                                s.text = label_clone.clone();
                                s.open = false;
                                s.refilter();
                            });
                            if let Some(ref handler) = on_select {
                                handler(hx);
                            }
                        }),
                    );

                    // Register blur handler for option
                    let state_clone = state.clone();
                    let base_id = id.clone();
                    registry.register(
                        &opt_id,
                        "on_blur",
                        Arc::new(move |hx| {
                            let should_close = match hx.blur_new_target() {
                                Some(new_target) => !new_target.starts_with(&base_id),
                                None => true,
                            };
                            if should_close {
                                state_clone.update(|s| s.open = false);
                            }
                        }),
                    );
                }
            }

            // Dropdown overlay positioning
            let dropdown_height = (current.filtered.len() as u16).min(10);

            let dropdown = options_col
                .position(Position::Absolute)
                .top(1)
                .left(-1)
                .padding(tuidom::Edges::left(1))
                .width(Size::Fixed(min_width + 1))
                .height(Size::Fixed(dropdown_height))
                .overflow(Overflow::Auto)
                .z_index(100)
                .style(Style::new().background(Color::var("autocomplete.dropdown_bg")));

            Element::box_()
                .width(Size::Fixed(min_width))
                .height(Size::Fixed(1))
                .child(input)
                .child(dropdown)
        } else {
            input
        }
    }
}
