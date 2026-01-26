//! Condition editor modal for creating/editing filter conditions.

use chrono::DateTime;
use dataverse_lib::model::Value;
use dataverse_lib::model::metadata::{AttributeMetadata, AttributeType};
use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{Autocomplete, AutocompleteState, Button, Input, Select, SelectState, Text};
use uuid::Uuid;

use crate::formatting::format_value;

use super::super::data::CondOp;

/// Result returned by the condition editor.
#[derive(Clone, Debug)]
pub struct ConditionData {
    pub field: String,
    pub operator: CondOp,
    pub value: Value,
}

/// Modal for creating or editing a filter condition.
#[modal(size = Md)]
pub struct ConditionEditorModal {
    #[state(skip)]
    options: Vec<(String, String)>,
    #[state(skip)]
    attributes: Vec<AttributeMetadata>,
    #[state(skip)]
    initial: Option<ConditionData>,

    field: AutocompleteState<String>,
    operator: SelectState<CondOp>,
    value_text: String,

    selected_type: Option<AttributeType>,
    error: Option<String>,
}

impl ConditionEditorModal {
    /// Create with pre-fetched field options and attribute metadata.
    pub fn new(options: Vec<(String, String)>, attributes: Vec<AttributeMetadata>) -> Self {
        Self {
            options,
            attributes,
            ..Default::default()
        }
    }

    /// Create pre-filled with an existing condition for editing.
    pub fn with_condition(
        options: Vec<(String, String)>,
        attributes: Vec<AttributeMetadata>,
        condition: ConditionData,
    ) -> Self {
        Self {
            options,
            attributes,
            initial: Some(condition),
            ..Default::default()
        }
    }
}

#[modal_impl]
impl ConditionEditorModal {
    fn default_result(&self) -> Option<ConditionData> {
        None
    }

    #[on_start]
    async fn on_start(&self, mx: &ModalContext<Option<ConditionData>>) {
        if let Some(initial) = &self.initial {
            // Pre-fill field autocomplete with selection
            self.field.set(
                AutocompleteState::new(self.options.clone()).with_value(initial.field.clone()),
            );

            // Determine attribute type for the field
            let attr_type = self
                .attributes
                .iter()
                .find(|a| a.logical_name == initial.field)
                .map(|a| a.attribute_type);
            self.selected_type.set(attr_type);

            // Pre-fill operator select with the correct options and selection
            let ops = operators_for_type(attr_type);
            let op_options: Vec<(CondOp, String)> =
                ops.iter().map(|op| (*op, op.label().to_string())).collect();
            self.operator
                .set(SelectState::new(op_options).with_value(initial.operator));

            // Pre-fill value text
            if initial.operator.has_value() {
                self.value_text.set(format_value(&initial.value).raw);
            }
        } else {
            self.field.set(AutocompleteState::new(self.options.clone()));
        }

        mx.focus("cond-field-autocomplete");
    }

    #[keybinds]
    fn keys() {
        bind("escape", cancel);
    }

    #[handler]
    async fn cancel(&self, mx: &ModalContext<Option<ConditionData>>) {
        mx.close(None);
    }

    #[handler]
    async fn confirm(&self, mx: &ModalContext<Option<ConditionData>>) {
        let field = self.field.with_ref(|s| s.value().cloned());
        let Some(field_name) = field else {
            return;
        };

        let op = self.operator.with_ref(|s| s.value().cloned());
        let Some(operator) = op else {
            return;
        };

        let value = if operator.has_value() {
            let text = self.value_text.get();
            let attr_type = self.selected_type.get();
            match parse_value(&text, attr_type) {
                Ok(v) => v,
                Err(msg) => {
                    self.error.set(Some(msg));
                    return;
                }
            }
        } else {
            Value::Null
        };

        mx.close(Some(ConditionData {
            field: field_name,
            operator,
            value,
        }));
    }

    #[derived]
    fn operator_options(&self) -> Vec<(CondOp, String)> {
        let attr_type = self.selected_type.get();
        let ops = operators_for_type(attr_type);
        ops.iter()
            .map(|op| (*op, op.label().to_string()))
            .collect::<Vec<_>>()
    }

    #[handler]
    async fn on_field_select(&self) {
        let field = self.field.with_ref(|s| s.value().cloned());
        let Some(field_name) = field else {
            return;
        };

        let attr_type = self
            .attributes
            .iter()
            .find(|a| a.logical_name == field_name)
            .map(|a| a.attribute_type);

        self.selected_type.set(attr_type);
        self.value_text.set(String::new());
        self.error.set(None);

        // Operator options update automatically via derived state
        let options = self.operator_options();
        self.operator.set(SelectState::new(options));
    }

    fn element(&self) -> Element {
        let error = self.error.get();
        let has_value_input = self
            .operator
            .with_ref(|s| s.value().map(|op| op.has_value()).unwrap_or(false));
        let type_hint = self
            .selected_type
            .get()
            .map(|t| type_hint_text(t))
            .unwrap_or_default();
        let title = if self.initial.is_some() {
            "Edit Condition"
        } else {
            "Add Condition"
        };

        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                text (content: {title}) style (bold, fg: interact)

                if let Some(err) = error {
                    text (content: {err}) style (fg: primary)
                }

                text (content: "Field") style (fg: muted)
                autocomplete (state: self.field, id: "cond-field-autocomplete", placeholder: "Search fields...")
                    on_select: on_field_select()
                text (content: "Operator") style (fg: muted)
                select (state: self.operator, id: "cond-operator", placeholder: "Select operator...")
                if has_value_input {
                    text (content: "Value") style (fg: muted)
                    input (state: self.value_text, id: "cond-value", placeholder: {type_hint})
                }

                row (width: fill, justify: between) {
                    button (label: "Cancel", hint: "esc", id: "cancel") on_activate: cancel()
                    button (label: "Ok", id: "ok") on_activate: confirm()
                }
            }
        }
    }
}

/// Get available operators for a given attribute type.
fn operators_for_type(attr_type: Option<AttributeType>) -> Vec<CondOp> {
    match attr_type {
        Some(AttributeType::String | AttributeType::Memo) => vec![
            CondOp::Eq,
            CondOp::Ne,
            CondOp::Contains,
            CondOp::StartsWith,
            CondOp::EndsWith,
            CondOp::IsNull,
            CondOp::IsNotNull,
        ],
        Some(
            AttributeType::Integer
            | AttributeType::BigInt
            | AttributeType::Double
            | AttributeType::Decimal
            | AttributeType::Money,
        ) => vec![
            CondOp::Eq,
            CondOp::Ne,
            CondOp::Gt,
            CondOp::Ge,
            CondOp::Lt,
            CondOp::Le,
            CondOp::IsNull,
            CondOp::IsNotNull,
        ],
        Some(AttributeType::Boolean) => {
            vec![CondOp::Eq, CondOp::Ne, CondOp::IsNull, CondOp::IsNotNull]
        }
        Some(AttributeType::DateTime) => vec![
            CondOp::Eq,
            CondOp::Ne,
            CondOp::Gt,
            CondOp::Ge,
            CondOp::Lt,
            CondOp::Le,
            CondOp::IsNull,
            CondOp::IsNotNull,
        ],
        Some(
            AttributeType::Uniqueidentifier
            | AttributeType::Lookup
            | AttributeType::Customer
            | AttributeType::Owner,
        ) => vec![CondOp::Eq, CondOp::Ne, CondOp::IsNull, CondOp::IsNotNull],
        Some(AttributeType::Picklist | AttributeType::State | AttributeType::Status) => {
            vec![CondOp::Eq, CondOp::Ne, CondOp::IsNull, CondOp::IsNotNull]
        }
        Some(AttributeType::MultiSelectPicklist) => vec![
            CondOp::Eq,
            CondOp::Ne,
            CondOp::Contains,
            CondOp::IsNull,
            CondOp::IsNotNull,
        ],
        _ => vec![CondOp::Eq, CondOp::Ne, CondOp::IsNull, CondOp::IsNotNull],
    }
}

/// Get placeholder hint text for a given attribute type.
fn type_hint_text(attr_type: AttributeType) -> String {
    match attr_type {
        AttributeType::String | AttributeType::Memo => "text value".to_string(),
        AttributeType::Integer | AttributeType::BigInt => "integer".to_string(),
        AttributeType::Double | AttributeType::Decimal | AttributeType::Money => {
            "number".to_string()
        }
        AttributeType::Boolean => "true or false".to_string(),
        AttributeType::DateTime => "2025-01-01T00:00:00Z".to_string(),
        AttributeType::Uniqueidentifier
        | AttributeType::Lookup
        | AttributeType::Customer
        | AttributeType::Owner => "guid".to_string(),
        AttributeType::Picklist
        | AttributeType::State
        | AttributeType::Status
        | AttributeType::MultiSelectPicklist => "option value (integer)".to_string(),
        _ => "value".to_string(),
    }
}

/// Parse a text value into the appropriate Value type.
fn parse_value(text: &str, attr_type: Option<AttributeType>) -> Result<Value, String> {
    if text.is_empty() {
        return Err("Value is required".to_string());
    }

    match attr_type {
        Some(AttributeType::String | AttributeType::Memo) => Ok(Value::String(text.to_string())),
        Some(AttributeType::Integer) => text
            .parse::<i32>()
            .map(Value::Int)
            .map_err(|_| "Invalid integer".to_string()),
        Some(AttributeType::BigInt) => text
            .parse::<i64>()
            .map(Value::Long)
            .map_err(|_| "Invalid integer".to_string()),
        Some(AttributeType::Double | AttributeType::Decimal | AttributeType::Money) => text
            .parse::<f64>()
            .map(Value::Float)
            .map_err(|_| "Invalid number".to_string()),
        Some(AttributeType::Boolean) => match text.to_lowercase().as_str() {
            "true" | "1" | "yes" => Ok(Value::Bool(true)),
            "false" | "0" | "no" => Ok(Value::Bool(false)),
            _ => Err("Enter true or false".to_string()),
        },
        Some(AttributeType::DateTime) => DateTime::parse_from_rfc3339(text)
            .map(|dt| Value::DateTime(dt.with_timezone(&chrono::Utc)))
            .or_else(|_| {
                chrono::NaiveDate::parse_from_str(text, "%Y-%m-%d")
                    .map(|d| Value::DateTime(d.and_hms_opt(0, 0, 0).unwrap().and_utc()))
            })
            .map_err(|_| "Invalid date (use YYYY-MM-DD or RFC3339)".to_string()),
        Some(
            AttributeType::Uniqueidentifier
            | AttributeType::Lookup
            | AttributeType::Customer
            | AttributeType::Owner,
        ) => text
            .parse::<Uuid>()
            .map(Value::Guid)
            .map_err(|_| "Invalid GUID format".to_string()),
        Some(AttributeType::Picklist | AttributeType::State | AttributeType::Status) => {
            let val: i32 = text
                .parse()
                .map_err(|_| "Invalid option value (integer)".to_string())?;
            Ok(Value::OptionSet(val.into()))
        }
        Some(AttributeType::MultiSelectPicklist) => {
            let vals: Result<Vec<i32>, _> =
                text.split(',').map(|s| s.trim().parse::<i32>()).collect();
            let vals = vals.map_err(|_| "Invalid values (comma-separated integers)".to_string())?;
            Ok(Value::MultiOptionSet(
                dataverse_lib::model::types::MultiSelectOptionSetValue::new(vals),
            ))
        }
        _ => Ok(Value::String(text.to_string())),
    }
}
