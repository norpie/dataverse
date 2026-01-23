//! Date picker widget - a dropdown calendar with optional time inputs.
//!
//! Renders as a compact date display when closed. When activated, opens a
//! dropdown calendar for date selection with optional hour/minute inputs.

use std::sync::Arc;

use chrono::{Datelike, NaiveDate, NaiveTime, Timelike, Utc};
use tuidom::{Color, Element, Overflow, Position, Size, Style, Transitions};

use crate::state::State;
use crate::{HandlerRegistry, WidgetHandlers};

use super::number_input::NumberInputState;

// =============================================================================
// DatePickerState
// =============================================================================

/// State for a date picker widget.
///
/// # Example
///
/// ```ignore
/// // In app struct (will be wrapped in State<> by #[app] macro):
/// start_date: DatePickerState,
///
/// // Initialize in on_start:
/// self.start_date.set(DatePickerState::new().with_time());
/// ```
#[derive(Clone, Debug)]
pub struct DatePickerState {
    /// The selected date.
    date: Option<NaiveDate>,
    /// Whether time inputs are shown.
    show_time: bool,
    /// Hour input state (0-23).
    hour: NumberInputState,
    /// Minute input state (0-59).
    minute: NumberInputState,
    /// Whether the dropdown is currently open.
    open: bool,
    /// The month currently being displayed (always the 1st of that month).
    viewing: NaiveDate,
}

impl Default for DatePickerState {
    fn default() -> Self {
        Self::new()
    }
}

impl DatePickerState {
    /// Create a new empty DatePickerState (no date selected).
    pub fn new() -> Self {
        let today = Utc::now().date_naive();
        Self {
            date: None,
            show_time: false,
            hour: NumberInputState::new(0.0)
                .with_min(0.0)
                .with_max(23.0)
                .with_step(1.0)
                .integer(),
            minute: NumberInputState::new(0.0)
                .with_min(0.0)
                .with_max(59.0)
                .with_step(1.0)
                .integer(),
            open: false,
            viewing: first_of_month(today),
        }
    }

    /// Create a DatePickerState with a pre-selected date.
    pub fn with_date(mut self, date: NaiveDate) -> Self {
        self.date = Some(date);
        self.viewing = first_of_month(date);
        self
    }

    /// Create a DatePickerState with a pre-selected date and time.
    pub fn with_datetime(mut self, date: NaiveDate, time: NaiveTime) -> Self {
        self.date = Some(date);
        self.viewing = first_of_month(date);
        self.show_time = true;
        self.hour.set_value(time.hour() as f64);
        self.minute.set_value(time.minute() as f64);
        self
    }

    /// Enable time selection.
    pub fn with_time(mut self) -> Self {
        self.show_time = true;
        self
    }

    /// Get the selected date.
    pub fn date(&self) -> Option<NaiveDate> {
        self.date
    }

    /// Get the selected time (if time mode is enabled).
    pub fn time(&self) -> Option<NaiveTime> {
        if self.show_time && self.date.is_some() {
            NaiveTime::from_hms_opt(self.hour.value() as u32, self.minute.value() as u32, 0)
        } else {
            None
        }
    }

    /// Get the combined date+time as a UTC datetime.
    pub fn datetime_utc(&self) -> Option<chrono::DateTime<Utc>> {
        let date = self.date?;
        let time = self
            .time()
            .unwrap_or_else(|| NaiveTime::from_hms_opt(0, 0, 0).unwrap());
        let naive_dt = date.and_time(time);
        Some(naive_dt.and_utc())
    }

    /// Set the date programmatically.
    pub fn set_date(&mut self, date: NaiveDate) {
        self.date = Some(date);
        self.viewing = first_of_month(date);
    }

    /// Set the date and time programmatically.
    pub fn set_datetime(&mut self, date: NaiveDate, time: NaiveTime) {
        self.date = Some(date);
        self.viewing = first_of_month(date);
        self.hour.set_value(time.hour() as f64);
        self.minute.set_value(time.minute() as f64);
    }

    /// Whether the dropdown is open.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Format the current date/time as a display string.
    fn display_text(&self) -> String {
        match self.date {
            Some(date) => {
                if self.show_time {
                    format!(
                        "{} {:02}:{:02}",
                        date.format("%Y-%m-%d"),
                        self.hour.value() as u32,
                        self.minute.value() as u32,
                    )
                } else {
                    date.format("%Y-%m-%d").to_string()
                }
            }
            None => String::new(),
        }
    }

    /// Navigate to the previous month.
    fn prev_month(&mut self) {
        self.viewing = prev_month(self.viewing);
    }

    /// Navigate to the next month.
    fn next_month(&mut self) {
        self.viewing = next_month(self.viewing);
    }

    /// Select a day in the currently viewed month.
    fn select_day(&mut self, day: u32) {
        if let Some(date) = self.viewing.with_day(day) {
            self.date = Some(date);
        }
    }
}

// =============================================================================
// Calendar Helpers
// =============================================================================

/// Get the first day of the month for a given date.
fn first_of_month(date: NaiveDate) -> NaiveDate {
    NaiveDate::from_ymd_opt(date.year(), date.month(), 1).unwrap_or(date)
}

/// Get the first day of the previous month.
fn prev_month(first: NaiveDate) -> NaiveDate {
    if first.month() == 1 {
        NaiveDate::from_ymd_opt(first.year() - 1, 12, 1).unwrap_or(first)
    } else {
        NaiveDate::from_ymd_opt(first.year(), first.month() - 1, 1).unwrap_or(first)
    }
}

/// Get the first day of the next month.
fn next_month(first: NaiveDate) -> NaiveDate {
    if first.month() == 12 {
        NaiveDate::from_ymd_opt(first.year() + 1, 1, 1).unwrap_or(first)
    } else {
        NaiveDate::from_ymd_opt(first.year(), first.month() + 1, 1).unwrap_or(first)
    }
}

/// Get the number of days in a month.
fn days_in_month(year: i32, month: u32) -> u32 {
    // Get the first day of the NEXT month, then subtract one day
    let next = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1)
    };
    next.map(|d| d.pred_opt().map(|p| p.day()).unwrap_or(28))
        .unwrap_or(28)
}

/// Get the weekday index (0=Monday, 6=Sunday) for a date.
fn weekday_index(date: NaiveDate) -> u32 {
    date.weekday().num_days_from_monday()
}

/// Month name for display.
fn month_name(month: u32) -> &'static str {
    match month {
        1 => "January",
        2 => "February",
        3 => "March",
        4 => "April",
        5 => "May",
        6 => "June",
        7 => "July",
        8 => "August",
        9 => "September",
        10 => "October",
        11 => "November",
        12 => "December",
        _ => "?",
    }
}

// =============================================================================
// Widget Builder
// =============================================================================

/// Typestate marker: date picker needs a state reference.
pub struct NeedsState;

/// Typestate marker: date picker has a state reference.
pub struct HasState<'a>(&'a State<DatePickerState>);

/// A date picker widget builder.
///
/// Uses typestate pattern to enforce `state()` is called before `build()`.
///
/// # Example
///
/// ```ignore
/// // In page! macro:
/// date_picker (state: self.start_date, id: "start-date", label: "Start Date")
///     on_change: date_changed()
/// ```
#[derive(Clone, Debug)]
pub struct DatePicker<S = NeedsState> {
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

impl Default for DatePicker<NeedsState> {
    fn default() -> Self {
        Self::new()
    }
}

impl DatePicker<NeedsState> {
    /// Create a new date picker builder.
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
    pub fn state(self, s: &State<DatePickerState>) -> DatePicker<HasState<'_>> {
        DatePicker {
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

impl<S> DatePicker<S> {
    /// Set the widget id.
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Set the placeholder text (shown when no date is selected).
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    /// Set the label text (displayed above the picker).
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Mark the picker as disabled.
    pub fn disabled(mut self) -> Self {
        self.disabled = true;
        self
    }

    /// Set the display width in characters.
    pub fn width(mut self, width: u16) -> Self {
        self.width = Some(width);
        self
    }

    /// Set the style.
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

impl<'a> DatePicker<HasState<'a>> {
    /// Build the date picker element.
    pub fn build(self, registry: &HandlerRegistry, handlers: &WidgetHandlers) -> Element {
        let state = self.state_marker.0;
        let current = state.get();
        let id = self.id.clone().unwrap_or_else(|| "date_picker".into());
        let display_width = self.width.unwrap_or(20);

        // Build the toggle display (always visible)
        let display_text = current.display_text();
        let toggle_text = if display_text.is_empty() {
            self.placeholder
                .as_deref()
                .unwrap_or("Select date...")
                .to_string()
        } else {
            display_text
        };

        let toggle_id = format!("{}-toggle", id);
        let is_placeholder = current.date.is_none();

        let toggle_style = if is_placeholder {
            Style::new()
                .background(Color::var("input.background"))
                .foreground(Color::var("input.placeholder"))
                .merge(&self.style)
        } else {
            Style::new()
                .background(Color::var("input.background"))
                .merge(&self.style)
        };

        let focused_style = Style::new()
            .background(Color::var("input.background").lighten(0.05))
            .merge(&self.style_focused);

        let mut toggle = Element::row()
            .id(&toggle_id)
            .width(Size::Fixed(display_width))
            .height(Size::Fixed(1))
            .focusable(!self.disabled)
            .clickable(!self.disabled)
            .child(Element::text(&toggle_text).width(Size::Fill))
            .child(Element::text(if current.open { "^" } else { "v" }))
            .style(toggle_style)
            .style_focused(focused_style);

        if let Some(s) = &self.style_disabled {
            toggle = toggle.style_disabled(s.clone());
        }

        if let Some(t) = self.transitions.clone() {
            toggle = toggle.transitions(t);
        }

        // Register toggle activate handler (open/close)
        if !self.disabled {
            let state_clone = state.clone();
            registry.register(
                &toggle_id,
                "on_activate",
                Arc::new(move |_hx| {
                    state_clone.update(|s| s.open = !s.open);
                }),
            );

            // Close on blur if focus leaves the widget entirely
            let state_clone = state.clone();
            let base_id = id.clone();
            registry.register(
                &toggle_id,
                "on_blur",
                Arc::new(move |hx| {
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
        }

        // Build dropdown if open
        let elem = if current.open && !self.disabled {
            let dropdown = self.build_dropdown(state, &current, &id, registry, handlers);

            // Full-screen backdrop
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

            let state_clone = state.clone();
            registry.register(
                &backdrop_id,
                "on_activate",
                Arc::new(move |_hx| {
                    state_clone.update(|s| s.open = false);
                }),
            );

            Element::box_()
                .width(Size::Fixed(display_width))
                .height(Size::Fixed(1))
                .child(toggle)
                .child(backdrop)
                .child(dropdown)
        } else {
            toggle
        };

        // Wrap in column with label if present
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

    /// Build the dropdown calendar.
    fn build_dropdown(
        &self,
        state: &State<DatePickerState>,
        current: &DatePickerState,
        id: &str,
        registry: &HandlerRegistry,
        handlers: &WidgetHandlers,
    ) -> Element {
        let dropdown_id = format!("{}-dropdown", id);
        let viewing = current.viewing;
        let year = viewing.year();
        let month = viewing.month();
        let days = days_in_month(year, month);
        let first_weekday = weekday_index(viewing); // 0=Mon, 6=Sun
        let today = Utc::now().date_naive();

        // Dropdown width: 7 days × 3 chars + 2 padding = 23
        let cal_width: u16 = 23;

        let mut dropdown_col = Element::col()
            .width(Size::Fixed(cal_width))
            .overflow(Overflow::Visible)
            .gap(0);

        // --- Header: < Month Year > ---
        let header = self.build_header(state, id, year, month, cal_width, registry);
        dropdown_col = dropdown_col.child(header);

        // --- Day-of-week labels ---
        let dow_labels = Element::row().width(Size::Fill).child(
            Element::text("Mo Tu We Th Fr Sa Su")
                .style(Style::new().foreground(Color::var("text.muted"))),
        );
        dropdown_col = dropdown_col.child(dow_labels);

        // --- Calendar grid ---
        let mut day = 1u32;
        for week in 0..6 {
            if day > days {
                break;
            }

            let mut row = Element::row().width(Size::Fill);

            for weekday in 0..7u32 {
                let cell_id = format!("{}-day-{}-{}", id, week, weekday);

                if (week == 0 && weekday < first_weekday) || day > days {
                    // Empty cell (before first day or after last day)
                    row = row.child(
                        Element::text("   ")
                            .id(&cell_id)
                            .width(Size::Fixed(3))
                            .height(Size::Fixed(1)),
                    );
                } else {
                    // Day cell
                    let current_day = day;
                    let is_selected = current.date == viewing.with_day(current_day);
                    let is_today = today == viewing.with_day(current_day).unwrap_or(today);
                    let day_text = format!("{:>2} ", current_day);

                    let mut cell = Element::row()
                        .id(&cell_id)
                        .width(Size::Fixed(3))
                        .height(Size::Fixed(1))
                        .focusable(true)
                        .clickable(true)
                        .child(Element::text(&day_text));

                    // Style: selected > today > normal
                    if is_selected {
                        cell = cell.style(
                            Style::new()
                                .background(Color::var("primary"))
                                .foreground(Color::var("text.inverted")),
                        );
                    } else if is_today {
                        cell = cell.style(Style::new().foreground(Color::var("interact")).bold());
                    }

                    cell = cell.style_focused(
                        Style::new()
                            .background(Color::var("list.item_focused"))
                            .foreground(Color::var("text.inverted")),
                    );

                    // Register activate handler for day selection
                    let state_clone = state.clone();
                    let on_change = handlers.get("on_change").cloned();
                    let show_time = current.show_time;
                    let time_id = format!("{}-hour", id);
                    registry.register(
                        &cell_id,
                        "on_activate",
                        Arc::new(move |hx| {
                            state_clone.update(|s| s.select_day(current_day));
                            if show_time {
                                // Focus the hour input for time entry
                                hx.cx().focus(&time_id);
                            } else {
                                // No time: close and fire on_change
                                state_clone.update(|s| s.open = false);
                                if let Some(ref handler) = on_change {
                                    handler(hx);
                                }
                            }
                        }),
                    );

                    // Close on blur if focus leaves widget
                    let state_clone = state.clone();
                    let base_id = id.to_string();
                    registry.register(
                        &cell_id,
                        "on_blur",
                        Arc::new(move |hx| {
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

                    row = row.child(cell);
                    day += 1;
                }
            }

            dropdown_col = dropdown_col.child(row);
        }

        // --- Time inputs (if show_time) ---
        if current.show_time {
            let time_row = self.build_time_row(state, id, current, registry, handlers);
            dropdown_col = dropdown_col.child(time_row);
        }

        dropdown_col
            .id(&dropdown_id)
            .position(Position::Absolute)
            .top(1)
            .left(0)
            .z_index(2)
            .interaction_scope(true)
            .padding(tuidom::Edges::horizontal(1))
            .style(Style::new().background(Color::var("select.dropdown_bg")))
    }

    /// Build the month/year header with navigation.
    fn build_header(
        &self,
        state: &State<DatePickerState>,
        id: &str,
        year: i32,
        month: u32,
        width: u16,
        registry: &HandlerRegistry,
    ) -> Element {
        let prev_id = format!("{}-prev", id);
        let next_id = format!("{}-next", id);

        let header_text = format!("{} {}", month_name(month), year);

        // Prev button
        let prev = Element::text("<")
            .id(&prev_id)
            .width(Size::Fixed(2))
            .focusable(true)
            .clickable(true)
            .style_focused(
                Style::new()
                    .background(Color::var("list.item_focused"))
                    .foreground(Color::var("text.inverted")),
            );

        let state_clone = state.clone();
        registry.register(
            &prev_id,
            "on_activate",
            Arc::new(move |_hx| {
                state_clone.update(|s| s.prev_month());
            }),
        );

        // Close on blur
        let state_clone = state.clone();
        let base_id = id.to_string();
        registry.register(
            &prev_id,
            "on_blur",
            Arc::new(move |hx| {
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

        // Next button
        let next = Element::text(">")
            .id(&next_id)
            .width(Size::Fixed(2))
            .focusable(true)
            .clickable(true)
            .style_focused(
                Style::new()
                    .background(Color::var("list.item_focused"))
                    .foreground(Color::var("text.inverted")),
            );

        let state_clone = state.clone();
        registry.register(
            &next_id,
            "on_activate",
            Arc::new(move |_hx| {
                state_clone.update(|s| s.next_month());
            }),
        );

        // Close on blur
        let state_clone = state.clone();
        let base_id = id.to_string();
        registry.register(
            &next_id,
            "on_blur",
            Arc::new(move |hx| {
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

        // Header label (centered)
        let label_width = width.saturating_sub(4); // 2 for prev + 2 for next
        let header_label = Element::text(&header_text)
            .width(Size::Fixed(label_width))
            .style(Style::new().foreground(Color::var("interact")).bold());

        Element::row()
            .width(Size::Fill)
            .height(Size::Fixed(1))
            .child(prev)
            .child(header_label)
            .child(next)
    }

    /// Build the time input row (hour:minute).
    fn build_time_row(
        &self,
        state: &State<DatePickerState>,
        id: &str,
        current: &DatePickerState,
        registry: &HandlerRegistry,
        handlers: &WidgetHandlers,
    ) -> Element {
        let hour_id = format!("{}-hour", id);
        let minute_id = format!("{}-minute", id);

        // Hour text input
        let hour_input = Element::text_input(current.hour.text())
            .id(&hour_id)
            .width(Size::Fixed(3))
            .focusable(true)
            .captures_input(true)
            .style(Style::new().background(Color::var("input.background")))
            .style_focused(Style::new().background(Color::var("input.background").lighten(0.05)))
            .placeholder("00");

        // Hour on_change: validate numeric input
        let state_clone = state.clone();
        registry.register(
            &hour_id,
            "on_change",
            Arc::new(move |hx| {
                if let Some(text) = hx.event().text() {
                    state_clone.update(|s| {
                        s.hour.handle_text_change(text);
                    });
                }
            }),
        );

        // Hour on_submit: finalize and focus minute
        let state_clone = state.clone();
        let min_id = minute_id.clone();
        registry.register(
            &hour_id,
            "on_submit",
            Arc::new(move |hx| {
                state_clone.update(|s| s.hour.finalize());
                hx.cx().focus(&min_id);
            }),
        );

        // Hour on_blur: finalize value
        let state_clone = state.clone();
        let base_id = id.to_string();
        registry.register(
            &hour_id,
            "on_blur",
            Arc::new(move |hx| {
                state_clone.update(|s| s.hour.finalize());
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

        // Minute text input
        let minute_input = Element::text_input(current.minute.text())
            .id(&minute_id)
            .width(Size::Fixed(3))
            .focusable(true)
            .captures_input(true)
            .style(Style::new().background(Color::var("input.background")))
            .style_focused(Style::new().background(Color::var("input.background").lighten(0.05)))
            .placeholder("00");

        // Minute on_change: validate numeric input
        let state_clone = state.clone();
        registry.register(
            &minute_id,
            "on_change",
            Arc::new(move |hx| {
                if let Some(text) = hx.event().text() {
                    state_clone.update(|s| {
                        s.minute.handle_text_change(text);
                    });
                }
            }),
        );

        // Minute on_submit: finalize, close, fire on_change
        let state_clone = state.clone();
        let on_change = handlers.get("on_change").cloned();
        registry.register(
            &minute_id,
            "on_submit",
            Arc::new(move |hx| {
                state_clone.update(|s| {
                    s.minute.finalize();
                    s.open = false;
                });
                if let Some(ref handler) = on_change {
                    handler(hx);
                }
            }),
        );

        // Minute on_blur: finalize value
        let state_clone = state.clone();
        let base_id = id.to_string();
        registry.register(
            &minute_id,
            "on_blur",
            Arc::new(move |hx| {
                state_clone.update(|s| s.minute.finalize());
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

        Element::row()
            .width(Size::Fill)
            .height(Size::Fixed(1))
            .gap(1)
            .child(Element::text("H:").style(Style::new().foreground(Color::var("text.muted"))))
            .child(hour_input)
            .child(Element::text("M:").style(Style::new().foreground(Color::var("text.muted"))))
            .child(minute_input)
    }
}
