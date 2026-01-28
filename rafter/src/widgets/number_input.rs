//! Number input widget - a single-line input for numeric values.
//!
//! Provides validation, min/max clamping, and step increment/decrement.

use std::sync::Arc;

use tuidom::{Color, Element, Style, Transitions};

use crate::state::State;
use crate::{HandlerRegistry, WidgetHandlers};

/// State for a number input widget.
///
/// Stores the current numeric value, its text representation, and configuration
/// for validation (min/max bounds, step, integer mode).
///
/// # Example
///
/// ```ignore
/// // In app struct (will be wrapped in State<> by #[app] macro):
/// batch_size: NumberInputState,
///
/// // Initialize in on_start:
/// self.batch_size.set(
///     NumberInputState::new(50.0)
///         .with_min(1.0)
///         .with_max(1000.0)
///         .with_step(10.0)
///         .integer()
/// );
/// ```
#[derive(Clone, Debug)]
pub struct NumberInputState {
    /// The current numeric value.
    value: f64,
    /// The text representation displayed in the input.
    text: String,
    /// Minimum allowed value.
    min: Option<f64>,
    /// Maximum allowed value.
    max: Option<f64>,
    /// Step for increment/decrement (default: 1.0).
    step: f64,
    /// If true, only integers are accepted (no decimal point).
    integer_only: bool,
    /// If true, negative values are allowed.
    allow_negative: bool,
}

impl Default for NumberInputState {
    fn default() -> Self {
        Self {
            value: 0.0,
            text: "0".to_string(),
            min: None,
            max: None,
            step: 1.0,
            integer_only: false,
            allow_negative: false,
        }
    }
}

impl NumberInputState {
    /// Create a new NumberInputState with the given initial value.
    pub fn new(value: f64) -> Self {
        let text = format_value(value, false);
        Self {
            value,
            text,
            ..Default::default()
        }
    }

    /// Set the minimum allowed value.
    pub fn with_min(mut self, min: f64) -> Self {
        self.min = Some(min);
        self
    }

    /// Set the maximum allowed value.
    pub fn with_max(mut self, max: f64) -> Self {
        self.max = Some(max);
        self
    }

    /// Set the step for increment/decrement.
    pub fn with_step(mut self, step: f64) -> Self {
        self.step = step;
        self
    }

    /// Set integer-only mode (rejects decimal points).
    pub fn integer(mut self) -> Self {
        self.integer_only = true;
        // Reformat text without decimals
        self.value = self.value.round();
        self.text = format_value(self.value, true);
        self
    }

    /// Allow negative values.
    pub fn allow_negative(mut self) -> Self {
        self.allow_negative = true;
        self
    }

    /// Get the current value as f64.
    pub fn value(&self) -> f64 {
        self.value
    }

    /// Get the current value as i32 (truncated).
    pub fn value_i32(&self) -> i32 {
        self.value as i32
    }

    /// Get the current value as i64 (truncated).
    pub fn value_i64(&self) -> i64 {
        self.value as i64
    }

    /// Get the current text representation.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Programmatically set the value.
    pub fn set_value(&mut self, value: f64) {
        self.value = self.clamp(value);
        self.text = format_value(self.value, self.integer_only);
    }

    /// Increment the value by one step.
    pub fn increment(&mut self) {
        self.set_value(self.value + self.step);
    }

    /// Decrement the value by one step.
    pub fn decrement(&mut self) {
        self.set_value(self.value - self.step);
    }

    /// Clamp a value to min/max bounds.
    fn clamp(&self, value: f64) -> f64 {
        let mut v = value;
        if let Some(min) = self.min
            && v < min {
                v = min;
            }
        if let Some(max) = self.max
            && v > max {
                v = max;
            }
        v
    }

    /// Check if a text string is a valid partial numeric input.
    ///
    /// A "partial" input is one that the user might still be typing, e.g.:
    /// - "" (empty, user cleared the field)
    /// - "-" (user started typing a negative number)
    /// - "." (user started typing a decimal)
    /// - "12." (user is about to type decimal digits)
    fn is_valid_partial(&self, text: &str) -> bool {
        if text.is_empty() {
            return true;
        }

        let mut has_dot = false;
        let mut first = true;

        for c in text.chars() {
            match c {
                '-' => {
                    if !first || !self.allow_negative {
                        return false;
                    }
                }
                '.' => {
                    if self.integer_only || has_dot {
                        return false;
                    }
                    has_dot = true;
                }
                '0'..='9' => {}
                _ => return false,
            }
            first = false;
        }

        true
    }

    /// Try to parse the current text into a final value.
    /// Returns the parsed and clamped value, or None if text is not a complete number.
    fn parse_and_clamp(&self, text: &str) -> Option<f64> {
        if text.is_empty() || text == "-" || text == "." || text == "-." {
            // Incomplete input - treat as the minimum or zero
            return Some(self.clamp(self.min.unwrap_or(0.0)));
        }

        let parsed = if self.integer_only {
            text.parse::<i64>().ok().map(|v| v as f64)
        } else {
            text.parse::<f64>().ok()
        };

        parsed.map(|v| {
            let v = if self.integer_only { v.round() } else { v };
            self.clamp(v)
        })
    }

    /// Handle a text change event. Updates internal state if the text is valid.
    /// Returns true if the state was updated (text accepted).
    ///
    /// Use this when embedding `NumberInputState` as a plain field inside another
    /// widget's state and using a raw `text_input` element.
    pub fn handle_text_change(&mut self, new_text: &str) -> bool {
        if !self.is_valid_partial(new_text) {
            return false;
        }

        self.text = new_text.to_string();

        // Try to parse a value from the partial text
        if let Some(value) = self.parse_and_clamp(new_text) {
            self.value = value;
        }

        true
    }

    /// Finalize the current text (called on submit/blur).
    /// Parses the text, clamps to bounds, and updates the display text.
    ///
    /// Use this when embedding `NumberInputState` as a plain field inside another
    /// widget's state and using a raw `text_input` element.
    pub fn finalize(&mut self) {
        if let Some(value) = self.parse_and_clamp(&self.text.clone()) {
            self.value = value;
            self.text = format_value(self.value, self.integer_only);
        } else {
            // If text is completely invalid, revert to current value
            self.text = format_value(self.value, self.integer_only);
        }
    }
}

/// Format a numeric value as a display string.
fn format_value(value: f64, integer_only: bool) -> String {
    if integer_only {
        format!("{}", value as i64)
    } else {
        // Remove trailing zeros but keep at least one decimal if it's a float
        let s = format!("{}", value);
        s
    }
}

// =============================================================================
// Widget Builder
// =============================================================================

/// Typestate marker: number input needs a state reference.
pub struct NeedsState;

/// Typestate marker: number input has a state reference.
pub struct HasState<'a>(&'a State<NumberInputState>);

/// A number input widget builder.
///
/// Uses typestate pattern to enforce `state()` is called before `build()`.
///
/// # Example
///
/// ```ignore
/// // In page! macro:
/// number_input (state: self.batch_size, id: "batch-size", label: "Batch Size")
///     on_change: batch_size_changed()
/// ```
#[derive(Clone, Debug)]
pub struct NumberInput<S = NeedsState> {
    state_marker: S,
    id: Option<String>,
    placeholder: Option<String>,
    label: Option<String>,
    disabled: bool,
    width: Option<u16>,
    style: Option<Style>,
    style_focused: Option<Style>,
    style_disabled: Option<Style>,
    transitions: Option<Transitions>,
}

impl Default for NumberInput<NeedsState> {
    fn default() -> Self {
        Self::new()
    }
}

impl NumberInput<NeedsState> {
    /// Create a new number input builder.
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
            transitions: None,
        }
    }

    /// Set the state reference. Required before calling `build()`.
    pub fn state(self, s: &State<NumberInputState>) -> NumberInput<HasState<'_>> {
        NumberInput {
            state_marker: HasState(s),
            id: self.id,
            placeholder: self.placeholder,
            label: self.label,
            disabled: self.disabled,
            width: self.width,
            style: self.style,
            style_focused: self.style_focused,
            style_disabled: self.style_disabled,
            transitions: self.transitions,
        }
    }
}

impl<S> NumberInput<S> {
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

    /// Set the label text (displayed above the input).
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
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

impl<'a> NumberInput<HasState<'a>> {
    /// Build the number input element.
    ///
    /// Registers `on_change` and `on_submit` handlers if provided and not disabled.
    /// The on_change handler validates numeric input and rejects invalid characters.
    /// The on_submit handler finalizes the value (parse + clamp to bounds).
    pub fn build(self, registry: &HandlerRegistry, handlers: &WidgetHandlers) -> Element {
        let state = self.state_marker.0;
        let current = state.get();
        let id = self.id.clone().unwrap_or_else(|| "number_input".into());

        // Build the text input element using the current text representation
        let mut elem = Element::text_input(&current.text)
            .id(&id)
            .focusable(!self.disabled)
            .captures_input(!self.disabled)
            .disabled(self.disabled);

        // Set width
        elem = match self.width {
            Some(w) => elem.width(tuidom::Size::Fixed(w)),
            None => elem.width(tuidom::Size::Fill),
        };

        if let Some(placeholder) = &self.placeholder {
            elem = elem.placeholder(placeholder);
        }

        // Styles
        let style = Style::new()
            .background(Color::var("input.background"))
            .merge(&self.style);
        let focused_style = Style::new()
            .background(Color::var("input.background").lighten(0.05))
            .merge(&self.style_focused);
        let disabled_style = Style::new()
            .background(Color::var("surface").darken(0.05))
            .merge(&self.style_disabled);
        elem = elem.style(style);
        elem = elem.style_focused(focused_style);
        elem = elem.style_disabled(disabled_style);

        if let Some(transitions) = self.transitions.clone() {
            elem = elem.transitions(transitions);
        }

        // Register handlers if not disabled
        if !self.disabled {
            // on_change: validate numeric input
            let state_clone = state.clone();
            let user_handler = handlers.get("on_change").cloned();
            registry.register(
                &id,
                "on_change",
                Arc::new(move |hx| {
                    if let Some(text) = hx.event().text() {
                        let mut accepted = false;
                        state_clone.update(|s| {
                            accepted = s.handle_text_change(text);
                        });
                        // Only call user handler if input was accepted
                        if accepted
                            && let Some(ref handler) = user_handler {
                                handler(hx);
                            }
                    }
                }),
            );

            // on_submit: finalize the value (parse + clamp)
            let state_clone = state.clone();
            let user_handler = handlers.get("on_submit").cloned();
            registry.register(
                &id,
                "on_submit",
                Arc::new(move |hx| {
                    state_clone.update(|s| s.finalize());
                    if let Some(ref handler) = user_handler {
                        handler(hx);
                    }
                }),
            );

            // on_blur: finalize the value when focus leaves
            let state_clone = state.clone();
            registry.register(
                &id,
                "on_blur",
                Arc::new(move |_hx| {
                    state_clone.update(|s| s.finalize());
                }),
            );

            // Note: Up/Down arrow increment/decrement is not implemented because
            // the focus system consumes Up/Down keys for navigation before they
            // reach widget handlers. The increment/decrement methods on
            // NumberInputState can still be used programmatically from app handlers.
        }

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
