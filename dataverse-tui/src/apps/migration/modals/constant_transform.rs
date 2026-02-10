//! Modal for editing a Constant transform.

use dataverse_lib::DataverseClient;
use dataverse_lib::model::Entity;
use dataverse_lib::model::Value;
use dataverse_lib::model::types::EntityReference;
use dataverse_lib::model::types::OptionSetValue;
use rafter::page;
use rafter::prelude::*;
use rafter::widgets::Autocomplete;
use rafter::widgets::AutocompleteState;
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
use uuid::Uuid;

use crate::modals::LoadingModal;

/// Value type options for constant transform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ConstantType {
    #[default]
    String,
    Number,
    Bool,
    Date,
    Lookup,
    OptionSet,
    Null,
}

impl ConstantType {
    fn label(&self) -> &'static str {
        match self {
            ConstantType::String => "String",
            ConstantType::Number => "Number",
            ConstantType::Bool => "Boolean",
            ConstantType::Date => "Date/Time",
            ConstantType::Lookup => "Lookup",
            ConstantType::OptionSet => "Option Set",
            ConstantType::Null => "Null",
        }
    }

    fn all() -> Vec<(ConstantType, std::string::String)> {
        vec![
            (ConstantType::String, "String".to_string()),
            (ConstantType::Number, "Number".to_string()),
            (ConstantType::Bool, "Boolean".to_string()),
            (ConstantType::Date, "Date/Time".to_string()),
            (ConstantType::Lookup, "Lookup".to_string()),
            (ConstantType::OptionSet, "Option Set".to_string()),
            (ConstantType::Null, "Null".to_string()),
        ]
    }
}

impl ToString for ConstantType {
    fn to_string(&self) -> std::string::String {
        self.label().to_string()
    }
}

/// Modal for editing a Constant transform's value.
#[modal(size = Md)]
pub struct ConstantTransformModal {
    /// Target environment client for metadata fetches.
    #[state(skip)]
    client: DataverseClient,

    /// Type selector.
    type_select: SelectState<ConstantType>,
    /// String value input.
    string_value: std::string::String,
    /// Number value input.
    number_value: NumberInputState,
    /// Boolean value.
    bool_value: bool,
    /// Date value input.
    date_value: DatePickerState,

    /// Lookup entity autocomplete.
    lookup_entity: AutocompleteState<std::string::String>,
    /// Lookup GUID.
    lookup_guid: std::string::String,

    /// Option set name autocomplete.
    option_set_name: AutocompleteState<std::string::String>,
    /// Option set value autocomplete (populated when an option set is selected).
    option_set_value: AutocompleteState<i32>,

    /// Validation error message.
    error: Option<std::string::String>,
}

impl ConstantTransformModal {
    /// Create a new Constant transform modal with the given initial value.
    pub fn new_modal(client: DataverseClient, current_value: Value) -> Self {
        let decomposed = Self::decompose_value(&current_value);

        let type_select =
            SelectState::new(ConstantType::all()).with_value(decomposed.constant_type);

        Self::new(
            client,
            type_select,
            decomposed.string_val,
            decomposed.number_state,
            decomposed.bool_val,
            decomposed.date_state,
            decomposed.lookup_entity,
            decomposed.lookup_guid,
            decomposed.option_set_name,
            decomposed.option_set_value,
            None,
        )
    }

    /// Decomposed value for initializing modal fields.
    fn decompose_value(value: &Value) -> DecomposedValue {
        let defaults = DecomposedValue::default();

        match value {
            Value::Null => DecomposedValue {
                constant_type: ConstantType::Null,
                ..defaults
            },
            Value::String(s) => DecomposedValue {
                constant_type: ConstantType::String,
                string_val: s.clone(),
                ..defaults
            },
            Value::Bool(b) => DecomposedValue {
                constant_type: ConstantType::Bool,
                bool_val: *b,
                ..defaults
            },
            Value::Int(n) => DecomposedValue {
                constant_type: ConstantType::Number,
                number_state: NumberInputState::new(*n as f64).allow_negative(),
                ..defaults
            },
            Value::Long(n) => DecomposedValue {
                constant_type: ConstantType::Number,
                number_state: NumberInputState::new(*n as f64).allow_negative(),
                ..defaults
            },
            Value::Float(n) => DecomposedValue {
                constant_type: ConstantType::Number,
                number_state: NumberInputState::new(*n).allow_negative(),
                ..defaults
            },
            Value::Decimal(n) => {
                use rust_decimal::prelude::ToPrimitive;
                DecomposedValue {
                    constant_type: ConstantType::Number,
                    number_state: NumberInputState::new(n.to_f64().unwrap_or(0.0)).allow_negative(),
                    ..defaults
                }
            }
            Value::DateTime(dt) => {
                let date = dt.date_naive();
                let time = dt.time();
                DecomposedValue {
                    constant_type: ConstantType::Date,
                    date_state: DatePickerState::new().with_datetime(date, time),
                    ..defaults
                }
            }
            Value::EntityReference(er) => DecomposedValue {
                constant_type: ConstantType::Lookup,
                lookup_entity: AutocompleteState::new(Vec::<(
                    std::string::String,
                    std::string::String,
                )>::new())
                .with_value(er.entity.name().to_string()),
                lookup_guid: er.id.to_string(),
                ..defaults
            },
            Value::OptionSet(os) => DecomposedValue {
                constant_type: ConstantType::OptionSet,
                option_set_value: AutocompleteState::new(vec![(
                    os.value,
                    os.label
                        .as_ref()
                        .map(|l| format!("{} ({})", l, os.value))
                        .unwrap_or_else(|| os.value.to_string()),
                )])
                .with_value(os.value),
                ..defaults
            },
            // For other types, default to string representation
            _ => DecomposedValue {
                constant_type: ConstantType::String,
                string_val: format!("{:?}", value),
                ..defaults
            },
        }
    }

    /// Get the currently selected type.
    fn selected_type(&self) -> ConstantType {
        self.type_select.get().value().copied().unwrap_or_default()
    }

    /// Build the Value from current inputs.
    fn build_value(&self) -> Result<Value, std::string::String> {
        match self.selected_type() {
            ConstantType::Null => Ok(Value::Null),
            ConstantType::String => Ok(Value::String(self.string_value.get().clone())),
            ConstantType::Bool => Ok(Value::Bool(self.bool_value.get().clone())),
            ConstantType::Number => {
                let number_state = self.number_value.get();
                let f = number_state.value();
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
            ConstantType::Lookup => {
                let entity = self
                    .lookup_entity
                    .with_ref(|s| s.value().cloned())
                    .unwrap_or_default();
                if entity.is_empty() {
                    return Err("Entity is required".to_string());
                }
                let guid_str = self.lookup_guid.get().trim().to_string();
                if guid_str.is_empty() {
                    return Err("GUID is required".to_string());
                }
                let id =
                    Uuid::parse_str(&guid_str).map_err(|_| "Invalid GUID format".to_string())?;
                Ok(Value::EntityReference(EntityReference::new(
                    Entity::logical(entity),
                    id,
                )))
            }
            ConstantType::OptionSet => {
                let Some(value) = self.option_set_value.with_ref(|s| s.value().copied()) else {
                    return Err("Select an option set value".to_string());
                };
                // Get the label from the selected option
                let label = self
                    .option_set_value
                    .with_ref(|s| s.selected_labels().first().map(|l| l.to_string()));
                Ok(Value::OptionSet(match label {
                    Some(l) => OptionSetValue::with_label(value, l),
                    None => OptionSetValue::new(value),
                }))
            }
        }
    }

    /// Check if lookup entities have been loaded.
    fn has_lookup_entities(&self) -> bool {
        self.lookup_entity.with_ref(|s| !s.options.is_empty())
    }

    /// Check if option set names have been loaded.
    fn has_option_set_names(&self) -> bool {
        self.option_set_name.with_ref(|s| !s.options.is_empty())
    }

    /// Check if option set values have been loaded.
    fn has_option_set_values(&self) -> bool {
        self.option_set_value.with_ref(|s| !s.options.is_empty())
    }
}

#[modal_impl]
impl ConstantTransformModal {
    fn default_result(&self) -> Option<Value> {
        None
    }

    #[on_start]
    async fn on_start(&self, mx: &ModalContext<Option<Value>>) {
        match self.selected_type() {
            ConstantType::String => mx.focus("string-input"),
            ConstantType::Number => mx.focus("number-input"),
            ConstantType::Date => mx.focus("date-input-toggle"),
            ConstantType::Lookup => mx.focus("lookup-entity-autocomplete"),
            ConstantType::OptionSet => mx.focus("os-name-autocomplete"),
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
    async fn on_type_change(&self, gx: &GlobalContext, mx: &ModalContext<Option<Value>>) {
        self.error.set(None);

        match self.selected_type() {
            ConstantType::String => mx.focus("string-input"),
            ConstantType::Number => mx.focus("number-input"),
            ConstantType::Bool => mx.focus("bool-checkbox"),
            ConstantType::Date => mx.focus("date-input-toggle"),
            ConstantType::Lookup => {
                if !self.has_lookup_entities() {
                    self.load_entities(gx).await;
                }
                mx.focus("lookup-entity-autocomplete");
            }
            ConstantType::OptionSet => {
                if !self.has_option_set_names() {
                    self.load_option_sets(gx).await;
                }
                mx.focus("os-name-autocomplete");
            }
            ConstantType::Null => {}
        }
    }

    #[handler]
    async fn on_value_change(&self, _mx: &ModalContext<Option<Value>>) {
        self.error.set(None);
    }

    #[handler]
    async fn on_os_name_change(&self, gx: &GlobalContext) {
        self.error.set(None);

        let Some(os_name) = self.option_set_name.with_ref(|s| s.value().cloned()) else {
            // Cleared — reset value autocomplete
            self.option_set_value.set(AutocompleteState::new(
                Vec::<(i32, std::string::String)>::new(),
            ));
            return;
        };

        // Fetch the selected option set's values
        let client = self.client.clone();
        let name = os_name.clone();
        let result = gx
            .modal(LoadingModal::run_with_default(
                "Loading option set values",
                || Err(dataverse_lib::error::Error::Cancelled),
                async move { client.metadata().global_option_set(&name).await },
            ))
            .await;

        match result {
            Ok(metadata) => {
                let options: Vec<(i32, std::string::String)> = metadata
                    .options
                    .iter()
                    .map(|o| {
                        let label = o.label.text().unwrap_or("").to_string();
                        let display = if label.is_empty() {
                            o.value.to_string()
                        } else {
                            format!("{} ({})", label, o.value)
                        };
                        (o.value, display)
                    })
                    .collect();
                self.option_set_value.set(AutocompleteState::new(options));
            }
            Err(e) if e.is_cancelled() => {}
            Err(e) => {
                log::error!("Failed to fetch option set '{}': {}", os_name, e);
                gx.toast(Toast::error("Failed to fetch option set values"));
            }
        }
    }

    /// Load target entities into the lookup entity autocomplete.
    async fn load_entities(&self, gx: &GlobalContext) {
        let client = self.client.clone();
        let result = gx
            .modal(LoadingModal::run_with_default(
                "Loading entities",
                || Err(dataverse_lib::error::Error::Cancelled),
                async move {
                    client.metadata().all_entities().await.map(|entities| {
                        let mut names: Vec<std::string::String> =
                            entities.into_iter().map(|e| e.logical_name).collect();
                        names.sort();
                        names
                    })
                },
            ))
            .await;

        match result {
            Ok(names) => {
                let options: Vec<(std::string::String, std::string::String)> =
                    names.into_iter().map(|name| (name.clone(), name)).collect();
                // Preserve current value if editing
                let current = self.lookup_entity.with_ref(|s| s.value().cloned());
                let mut state = AutocompleteState::new(options);
                if let Some(val) = current {
                    state = state.with_value(val);
                }
                self.lookup_entity.set(state);
            }
            Err(e) if e.is_cancelled() => {}
            Err(e) => {
                log::error!("Failed to fetch entities: {}", e);
                gx.toast(Toast::error("Failed to fetch entity list"));
            }
        }
    }

    /// Load global option set names into the option set name autocomplete.
    async fn load_option_sets(&self, gx: &GlobalContext) {
        let client = self.client.clone();
        let result = gx
            .modal(LoadingModal::run_with_default(
                "Loading option sets",
                || Err(dataverse_lib::error::Error::Cancelled),
                async move {
                    client
                        .metadata()
                        .all_global_option_sets()
                        .await
                        .map(|option_sets| {
                            let mut names: Vec<std::string::String> =
                                option_sets.into_iter().map(|os| os.name).collect();
                            names.sort();
                            names
                        })
                },
            ))
            .await;

        match result {
            Ok(names) => {
                let options: Vec<(std::string::String, std::string::String)> =
                    names.into_iter().map(|name| (name.clone(), name)).collect();
                self.option_set_name.set(AutocompleteState::new(options));
            }
            Err(e) if e.is_cancelled() => {}
            Err(e) => {
                log::error!("Failed to fetch option sets: {}", e);
                gx.toast(Toast::error("Failed to fetch option set list"));
            }
        }
    }

    fn element(&self) -> Element {
        let selected_type = self.selected_type();
        let error = self.error.get();
        let has_os_values = self.has_option_set_values();

        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                text (content: "Constant") style (bold, fg: interact)

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
                    ConstantType::Lookup => {
                        column (gap: 1, width: fill) {
                            column (gap: 0, width: fill) {
                                text (content: "Entity") style (fg: muted)
                                autocomplete (
                                    state: self.lookup_entity,
                                    id: "lookup-entity-autocomplete",
                                    placeholder: "Search entities..."
                                )
                                    on_change: on_value_change()
                            }
                            column (gap: 0, width: fill) {
                                text (content: "Record GUID") style (fg: muted)
                                input (
                                    state: self.lookup_guid,
                                    id: "lookup-guid-input",
                                    placeholder: "00000000-0000-0000-0000-000000000000",
                                    width: fill
                                )
                                    on_change: on_value_change()
                            }
                        }
                    }
                    ConstantType::OptionSet => {
                        column (gap: 1, width: fill) {
                            column (gap: 0, width: fill) {
                                text (content: "Option Set") style (fg: muted)
                                autocomplete (
                                    state: self.option_set_name,
                                    id: "os-name-autocomplete",
                                    placeholder: "Search option sets..."
                                )
                                    on_submit: on_os_name_change()
                            }
                            if has_os_values {
                                column (gap: 0, width: fill) {
                                    text (content: "Value") style (fg: muted)
                                    autocomplete (
                                        state: self.option_set_value,
                                        id: "os-value-autocomplete",
                                        placeholder: "Search values..."
                                    )
                                        on_change: on_value_change()
                                }
                            }
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

/// Helper struct for decomposed value fields.
struct DecomposedValue {
    constant_type: ConstantType,
    string_val: std::string::String,
    number_state: NumberInputState,
    bool_val: bool,
    date_state: DatePickerState,
    lookup_entity: AutocompleteState<std::string::String>,
    lookup_guid: std::string::String,
    option_set_name: AutocompleteState<std::string::String>,
    option_set_value: AutocompleteState<i32>,
}

impl Default for DecomposedValue {
    fn default() -> Self {
        Self {
            constant_type: ConstantType::Null,
            string_val: std::string::String::new(),
            number_state: NumberInputState::new(0.0).allow_negative(),
            bool_val: false,
            date_state: DatePickerState::new().with_time(),
            lookup_entity: AutocompleteState::new(
                Vec::<(std::string::String, std::string::String)>::new(),
            ),
            lookup_guid: std::string::String::new(),
            option_set_name: AutocompleteState::new(Vec::<(
                std::string::String,
                std::string::String,
            )>::new()),
            option_set_value: AutocompleteState::new(Vec::<(i32, std::string::String)>::new()),
        }
    }
}
