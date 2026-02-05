//! Modal for editing a Constant transform.

use chrono::DateTime;
use chrono::NaiveDateTime;
use chrono::Utc;
use dataverse_lib::model::Value;
use rafter::page;
use rafter::prelude::*;
use rafter::widgets::Button;
use rafter::widgets::Checkbox;
use rafter::widgets::Input;
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
    number_value: String,
    /// Boolean value.
    bool_value: bool,
    /// Date value input (ISO format).
    date_value: String,
    /// Validation error message.
    error: Option<String>,
}

impl ConstantTransformModal {
    /// Create a new Constant transform modal with the given initial value.
    pub fn new_modal(current_value: Value) -> Self {
        let (constant_type, string_val, number_val, bool_val, date_val) =
            Self::decompose_value(&current_value);

        let type_select = SelectState::new(ConstantType::all()).with_value(constant_type);

        Self::new(
            type_select,
            string_val,
            number_val,
            bool_val,
            date_val,
            None,
        )
    }

    /// Decompose a Value into type and component values.
    fn decompose_value(value: &Value) -> (ConstantType, String, String, bool, String) {
        match value {
            Value::Null => (ConstantType::Null, String::new(), String::new(), false, String::new()),
            Value::String(s) => (ConstantType::String, s.clone(), String::new(), false, String::new()),
            Value::Bool(b) => (ConstantType::Bool, String::new(), String::new(), *b, String::new()),
            Value::Int(n) => (ConstantType::Number, String::new(), n.to_string(), false, String::new()),
            Value::Long(n) => (ConstantType::Number, String::new(), n.to_string(), false, String::new()),
            Value::Float(n) => (ConstantType::Number, String::new(), n.to_string(), false, String::new()),
            Value::Decimal(n) => (ConstantType::Number, String::new(), n.to_string(), false, String::new()),
            Value::DateTime(dt) => (ConstantType::Date, String::new(), String::new(), false, dt.format("%Y-%m-%dT%H:%M:%S").to_string()),
            // For other types, default to string representation
            _ => (ConstantType::String, format!("{:?}", value), String::new(), false, String::new()),
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
                let text = self.number_value.get().trim().to_string();
                if text.is_empty() {
                    return Err("Number value is required".to_string());
                }
                // Try parsing as decimal (most flexible)
                text.parse::<Decimal>()
                    .map(Value::Decimal)
                    .map_err(|_| format!("Invalid number: {}", text))
            }
            ConstantType::Date => {
                let text = self.date_value.get().trim().to_string();
                if text.is_empty() {
                    return Err("Date value is required".to_string());
                }
                // Try parsing as ISO datetime
                NaiveDateTime::parse_from_str(&text, "%Y-%m-%dT%H:%M:%S")
                    .map(|dt| Value::DateTime(DateTime::from_naive_utc_and_offset(dt, Utc)))
                    .or_else(|_| {
                        // Try date-only format
                        chrono::NaiveDate::parse_from_str(&text, "%Y-%m-%d")
                            .map(|d| {
                                let dt = d.and_hms_opt(0, 0, 0).unwrap();
                                Value::DateTime(DateTime::from_naive_utc_and_offset(dt, Utc))
                            })
                    })
                    .map_err(|_| "Invalid date format. Use YYYY-MM-DD or YYYY-MM-DDTHH:MM:SS".to_string())
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
            ConstantType::Date => mx.focus("date-input"),
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
            ConstantType::Date => mx.focus("date-input"),
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
                            input (
                                state: self.number_value,
                                id: "number-input",
                                placeholder: "Enter number (e.g., 42, 3.14)",
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
                            input (
                                state: self.date_value,
                                id: "date-input",
                                placeholder: "YYYY-MM-DD or YYYY-MM-DDTHH:MM:SS",
                                width: fill
                            )
                                on_change: on_value_change()
                            text (content: "Format: YYYY-MM-DD or YYYY-MM-DDTHH:MM:SS") style (fg: muted)
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
