//! Condition editor modal for creating/editing filter conditions.

use dataverse_lib::model::Value;
use dataverse_lib::model::metadata::{AttributeType, EntityMetadata};
use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{
    Autocomplete, AutocompleteState, Button, DatePicker, DatePickerState, Input, NumberInput,
    NumberInputState, Select, SelectState, Text,
};

use crate::formatting::{format_value, parse_filter_value, type_hint_text};

use super::types::{CondOp, ConditionData};

/// Modal for creating or editing a filter condition.
#[modal(size = Md)]
pub struct ConditionEditorModal {
    #[state(skip)]
    options: Vec<(String, String)>,
    #[state(skip)]
    metadata: EntityMetadata,
    #[state(skip)]
    initial: Option<ConditionData>,

    field: AutocompleteState<String>,
    operator: SelectState<CondOp>,
    value_text: String,
    date_picker: DatePickerState,
    number_input: NumberInputState,
    optionset_select: SelectState<i32>,

    selected_type: Option<AttributeType>,
    error: Option<String>,
}

impl ConditionEditorModal {
    /// Create with pre-fetched field options and entity metadata.
    pub fn with_options(options: Vec<(String, String)>, metadata: EntityMetadata) -> Self {
        Self::new(
            options,
            metadata,
            None,
            AutocompleteState::default(),
            SelectState::default(),
            String::new(),
            DatePickerState::default(),
            NumberInputState::default(),
            SelectState::default(),
            None,
            None,
        )
    }

    /// Create pre-filled with an existing condition for editing.
    pub fn with_condition(
        options: Vec<(String, String)>,
        metadata: EntityMetadata,
        condition: ConditionData,
    ) -> Self {
        Self::new(
            options,
            metadata,
            Some(condition),
            AutocompleteState::default(),
            SelectState::default(),
            String::new(),
            DatePickerState::default(),
            NumberInputState::default(),
            SelectState::default(),
            None,
            None,
        )
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
                .metadata
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

            // Pre-fill value in appropriate widget
            if initial.operator.has_value() {
                match attr_type {
                    Some(AttributeType::DateTime) => {
                        if let Value::DateTime(dt) = &initial.value {
                            self.date_picker.set(
                                DatePickerState::new()
                                    .with_datetime(dt.date_naive(), dt.time())
                                    .with_time(),
                            );
                        }
                    }
                    Some(AttributeType::Integer) => {
                        if let Value::Int(n) = &initial.value {
                            self.number_input
                                .set(NumberInputState::new(*n as f64).integer());
                        }
                    }
                    Some(AttributeType::BigInt) => {
                        if let Value::Long(n) = &initial.value {
                            self.number_input
                                .set(NumberInputState::new(*n as f64).integer());
                        }
                    }
                    Some(AttributeType::Double | AttributeType::Decimal | AttributeType::Money) => {
                        if let Value::Float(n) = &initial.value {
                            self.number_input.set(NumberInputState::new(*n));
                        }
                    }
                    Some(
                        AttributeType::Picklist | AttributeType::State | AttributeType::Status,
                    ) => {
                        let os_options = optionset_options(&self.metadata, &initial.field);
                        if let Value::OptionSet(os) = &initial.value {
                            self.optionset_select
                                .set(SelectState::new(os_options).with_value(os.value));
                        } else {
                            self.optionset_select.set(SelectState::new(os_options));
                        }
                    }
                    _ => {
                        // Text input - set formatted value
                        self.value_text.set(format_value(&initial.value).raw);
                    }
                }
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
            let attr_type = self.selected_type.get();
            match attr_type {
                Some(AttributeType::DateTime) => {
                    match self.date_picker.with_ref(|s| s.datetime_utc()) {
                        Some(dt) => Value::DateTime(dt),
                        None => {
                            self.error.set(Some("Please select a date".to_string()));
                            return;
                        }
                    }
                }
                Some(AttributeType::Integer) => {
                    Value::Int(self.number_input.with_ref(|s| s.value_i32()))
                }
                Some(AttributeType::BigInt) => {
                    Value::Long(self.number_input.with_ref(|s| s.value_i64()))
                }
                Some(AttributeType::Double | AttributeType::Decimal | AttributeType::Money) => {
                    Value::Float(self.number_input.with_ref(|s| s.value()))
                }
                Some(AttributeType::Picklist | AttributeType::State | AttributeType::Status) => {
                    match self.optionset_select.with_ref(|s| s.value().cloned()) {
                        Some(v) => Value::OptionSet(v.into()),
                        None => {
                            self.error.set(Some("Please select an option".to_string()));
                            return;
                        }
                    }
                }
                _ => {
                    // Fall back to text parsing for other types (String, GUID, etc.)
                    let text = self.value_text.get();
                    match parse_filter_value(&text, attr_type) {
                        Ok(v) => v,
                        Err(e) => {
                            self.error.set(Some(e.to_string()));
                            return;
                        }
                    }
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
            .metadata
            .attributes
            .iter()
            .find(|a| a.logical_name == field_name)
            .map(|a| a.attribute_type);

        self.selected_type.set(attr_type);
        self.value_text.set(String::new());
        self.error.set(None);

        // Initialize appropriate widget based on type
        match attr_type {
            Some(AttributeType::DateTime) => {
                self.date_picker.set(DatePickerState::new().with_time());
            }
            Some(AttributeType::Integer | AttributeType::BigInt) => {
                self.number_input.set(NumberInputState::new(0.0).integer());
            }
            Some(AttributeType::Double | AttributeType::Decimal | AttributeType::Money) => {
                self.number_input.set(NumberInputState::new(0.0));
            }
            Some(AttributeType::Picklist | AttributeType::State | AttributeType::Status) => {
                let os_options = optionset_options(&self.metadata, &field_name);
                self.optionset_select.set(SelectState::new(os_options));
            }
            _ => {
                // Text input - already cleared above
            }
        }

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
            .map(type_hint_text)
            .unwrap_or("value");
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

                    match self.selected_type.get() {
                        Some(AttributeType::DateTime) => {
                            date_picker (state: self.date_picker, id: "cond-value")
                        }
                        Some(
                            AttributeType::Integer
                            | AttributeType::BigInt
                            | AttributeType::Double
                            | AttributeType::Decimal
                            | AttributeType::Money
                        ) => {
                            number_input (state: self.number_input, id: "cond-value")
                        }
                        Some(
                            AttributeType::Picklist
                            | AttributeType::State
                            | AttributeType::Status
                        ) => {
                            select (state: self.optionset_select, id: "cond-value", placeholder: "Select option...")
                        }
                        _ => {
                            input (state: self.value_text, id: "cond-value", placeholder: {type_hint})
                        }
                    }
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
pub fn operators_for_type(attr_type: Option<AttributeType>) -> Vec<CondOp> {
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

/// Build `(value, "Display Name (value)")` pairs for an optionset attribute
/// by looking up the typed attribute lists in `EntityMetadata`.
fn optionset_options(metadata: &EntityMetadata, field_name: &str) -> Vec<(i32, String)> {
    // Try picklist attributes first
    if let Some(attr) = metadata.picklist_attribute(field_name) {
        return attr
            .option_set
            .options
            .iter()
            .map(|o| {
                let label = o.label.text_or("(unnamed)");
                (o.value, format!("{} ({})", label, o.value))
            })
            .collect();
    }

    // Try state attributes
    if let Some(attr) = metadata.state_attribute(field_name) {
        return attr
            .option_set
            .options
            .iter()
            .map(|o| {
                let label = o.label.text_or("(unnamed)");
                (o.value, format!("{} ({})", label, o.value))
            })
            .collect();
    }

    // Try status attributes
    if let Some(attr) = metadata.status_attribute(field_name) {
        return attr
            .option_set
            .options
            .iter()
            .map(|o| {
                let label = o.label.text_or("(unnamed)");
                (o.value, format!("{} ({})", label, o.value))
            })
            .collect();
    }

    Vec::new()
}
