//! Modal for editing a Constant transform.

use dataverse_lib::model::Value;
use rafter::page;
use rafter::prelude::*;
use rafter::widgets::Button;
use rafter::widgets::Checkbox;
use rafter::widgets::DatePicker;
use rafter::widgets::DatePickerState;
use rafter::widgets::Input;
use rafter::widgets::NumberInput;
use rafter::widgets::NumberInputState;
use rafter::widgets::Select;
use rafter::widgets::SelectState;
use rafter::widgets::Text;
use rust_decimal::Decimal;
use tuidom::Element;

/// Value type options for constant transform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ConstantType {
    #[default]
    String,
    Number,
    Bool,
    Date,
    Null,
}

impl ConstantType {
    fn label(&self) -> &'static str {
        match self {
            ConstantType::String => "String",
            ConstantType::Number => "Number",
            ConstantType::Bool => "Boolean",
            ConstantType::Date => "Date/Time",
            ConstantType::Null => "Null",
        }
    }

    fn all() -> Vec<(ConstantType, String)> {
        vec![
            (ConstantType::String, "String".to_string()),
            (ConstantType::Number, "Number".to_string()),
            (ConstantType::Bool, "Boolean".to_string()),
            (ConstantType::Date, "Date/Time".to_string()),
            (ConstantType::Null, "Null".to_string()),
        ]
    }
}

impl ToString for ConstantType {
    fn to_string(&self) -> String {
        self.label().to_string()
    }
}

/// Modal for editing a Constant transform's value.
#[modal(size = Sm)]
pub struct ConstantTransformModal {
    /// Type selector.
    type_select: SelectState<ConstantType>,
    /// String value input.
    string_value: String,
    /// Number value input.
    number_value: NumberInputState,
    /// Boolean value.
    bool_value: bool,
    /// Date value input.
    date_value: DatePickerState,
    /// Validation error message.
    error: Option<String>,
}

impl ConstantTransformModal {
    /// Create a new Constant transform modal with the given initial value.
    pub fn new_modal(current_value: Value) -> Self {
        let (constant_type, string_val, number_state, bool_val, date_state) =
            Self::decompose_value(&current_value);

        let type_select = SelectState::new(ConstantType::all()).with_value(constant_type);

        Self::new(
            type_select,
            string_val,
            number_state,
            bool_val,
            date_state,
            None,
        )
    }

    /// Decompose a Value into type and component values.
    fn decompose_value(
        value: &Value,
    ) -> (ConstantType, String, NumberInputState, bool, DatePickerState) {
        let default_number = NumberInputState::new(0.0).allow_negative();
        let default_date = DatePickerState::new().with_time();

        match value {
            Value::Null => (
                ConstantType::Null,
                String::new(),
                default_number,
                false,
                default_date,
            ),
            Value::String(s) => (
                ConstantType::String,
                s.clone(),
                default_number,
                false,
                default_date,
            ),
            Value::Bool(b) => (
                ConstantType::Bool,
                String::new(),
                default_number,
                *b,
                default_date,
            ),
            Value::Int(n) => (
                ConstantType::Number,
                String::new(),
                NumberInputState::new(*n as f64).allow_negative(),
                false,
                default_date,
            ),
            Value::Long(n) => (
                ConstantType::Number,
                String::new(),
                NumberInputState::new(*n as f64).allow_negative(),
                false,
                default_date,
            ),
            Value::Float(n) => (
                ConstantType::Number,
                String::new(),
                NumberInputState::new(*n).allow_negative(),
                false,
                default_date,
            ),
            Value::Decimal(n) => {
                use rust_decimal::prelude::ToPrimitive;
                let f = n.to_f64().unwrap_or(0.0);
                (
                    ConstantType::Number,
                    String::new(),
                    NumberInputState::new(f).allow_negative(),
                    false,
                    default_date,
                )
            }
            Value::DateTime(dt) => {
                let date = dt.date_naive();
                let time = dt.time();
                (
                    ConstantType::Date,
                    String::new(),
                    default_number,
                    false,
                    DatePickerState::new().with_datetime(date, time),
                )
            }
            // For other types, default to string representation
            _ => (
                ConstantType::String,
                format!("{:?}", value),
                default_number,
                false,
                default_date,
            ),
        }
    }

    /// Get the currently selected type.
    fn selected_type(&self) -> ConstantType {
        self.type_select.get().value().copied().unwrap_or_default()
    }

    /// Build the Value from current inputs.
    fn build_value(&self) -> Result<Value, String> {
        match self.selected_type() {
            ConstantType::Null => Ok(Value::Null),
            ConstantType::String => Ok(Value::String(self.string_value.get().clone())),
            ConstantType::Bool => Ok(Value::Bool(self.bool_value.get().clone())),
            ConstantType::Number => {
                let number_state = self.number_value.get();
                let f = number_state.value();
                // Convert f64 to Decimal for precision
                let text = format!("{}", f);
                text.parse::<Decimal>()
                    .map(Value::Decimal)
                    .map_err(|_| format!("Invalid number: {}", text))
            }
            ConstantType::Date => {
                let date_state = self.date_value.get();
                match date_state.datetime_utc() {
                    Some(dt) => Ok(Value::DateTime(dt)),
                    None => Err("Please select a date".to_string()),
                }
            }
        }
    }
}

#[modal_impl]
impl ConstantTransformModal {
    fn default_result(&self) -> Option<Value> {
        None
    }

    #[on_start]
    async fn on_start(&self, mx: &ModalContext<Option<Value>>) {
        // Focus appropriate input based on type
        match self.selected_type() {
            ConstantType::String => mx.focus("string-input"),
            ConstantType::Number => mx.focus("number-input"),
            ConstantType::Date => mx.focus("date-input-toggle"),
            _ => mx.focus("type-select"),
        }
    }

    #[keybinds]
    fn keybinds() {
        bind("escape", cancel);
        bind("ctrl+s", save);
    }

    #[handler]
    async fn cancel(&self, mx: &ModalContext<Option<Value>>) {
        mx.close(None);
    }

    #[handler]
    async fn save(&self, mx: &ModalContext<Option<Value>>) {
        match self.build_value() {
            Ok(value) => mx.close(Some(value)),
            Err(e) => self.error.set(Some(e)),
        }
    }

    #[handler]
    async fn on_type_change(&self, mx: &ModalContext<Option<Value>>) {
        // Clear error when type changes
        self.error.set(None);

        // Focus appropriate input
        match self.selected_type() {
            ConstantType::String => mx.focus("string-input"),
            ConstantType::Number => mx.focus("number-input"),
            ConstantType::Bool => mx.focus("bool-checkbox"),
            ConstantType::Date => mx.focus("date-input-toggle"),
            ConstantType::Null => {}
        }
    }

    #[handler]
    async fn on_value_change(&self, _mx: &ModalContext<Option<Value>>) {
        // Clear error when value changes
        self.error.set(None);
    }

    fn element(&self) -> Element {
        let selected_type = self.selected_type();
        let error = self.error.get();

        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                text (content: "Edit Constant Transform") style (bold, fg: interact)

                // Type selector
                column (gap: 0, width: fill) {
                    text (content: "Type") style (fg: muted)
                    select (
                        state: self.type_select,
                        id: "type-select",
                        width: fill
                    )
                        on_change: on_type_change()
                }

                // Value input (type-specific)
                match selected_type {
                    ConstantType::String => {
                        column (gap: 0, width: fill) {
                            text (content: "Value") style (fg: muted)
                            input (
                                state: self.string_value,
                                id: "string-input",
                                placeholder: "Enter string value",
                                width: fill
                            )
                                on_change: on_value_change()
                        }
                    }
                    ConstantType::Number => {
                        column (gap: 0, width: fill) {
                            text (content: "Value") style (fg: muted)
                            number_input (
                                state: self.number_value,
                                id: "number-input",
                                placeholder: "0",
                                width: fill
                            )
                                on_change: on_value_change()
                        }
                    }
                    ConstantType::Bool => {
                        row (gap: 1, width: fill) {
                            checkbox (
                                state: self.bool_value,
                                id: "bool-checkbox",
                                label: "Value (checked = true)"
                            )
                        }
                    }
                    ConstantType::Date => {
                        column (gap: 0, width: fill) {
                            text (content: "Value") style (fg: muted)
                            date_picker (
                                state: self.date_value,
                                id: "date-input",
                                placeholder: "Select date...",
                                width: fill
                            )
                                on_change: on_value_change()
                        }
                    }
                    ConstantType::Null => {
                        text (content: "Value will be null (no input needed)") style (fg: muted)
                    }
                }

                // Error message
                if let Some(err) = error {
                    text (content: {&err}) style (fg: error)
                }

                // Spacer
                box_ (height: fill) {}

                // Buttons
                row (width: fill, justify: between) {
                    button (label: "Cancel", hint: "esc", id: "cancel-btn")
                        on_activate: cancel()
                    button (label: "Save", hint: "ctrl+s", id: "save-btn")
                        on_activate: save()
                }
            }
        }
    }
}
