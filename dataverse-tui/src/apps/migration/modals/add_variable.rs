//! Modal for adding or editing a variable.

use dataverse_lib::model::FieldType;
use dataverse_lib::model::ValueType;
use dataverse_lib::model::metadata::AttributeType;
use dataverse_lib::DataverseClient;
use rafter::page;
use rafter::prelude::*;
use rafter::widgets::Button;
use rafter::widgets::Input;
use rafter::widgets::List;
use rafter::widgets::ListItem;
use rafter::widgets::ListState;
use rafter::widgets::Text;
use tuidom::Element;

use super::LookupTypePickerModal;
use super::OptionSetTypePickerModal;
use super::SimpleTypePickerModal;
use super::TypeCategory;
use super::TypeCategoryPickerModal;
use crate::modals::LoadingModal;

/// Result of the add/edit variable modal.
#[derive(Debug, Clone)]
pub struct AddVariableResult {
    pub name: String,
    pub declared_type: ValueType,
}

/// Wrapper around `FieldType` for the inline type list.
#[derive(Debug, Clone, PartialEq, Eq)]
struct FieldTypeEntry(FieldType);

impl ListItem for FieldTypeEntry {
    type Key = String;

    fn key(&self) -> String {
        self.0.display()
    }

    fn render(&self) -> Element {
        Element::text(self.0.display())
    }
}

/// Modal for adding or editing a variable.
#[modal(size = Md)]
pub struct AddVariableModal {
    #[state(skip)]
    client: DataverseClient,
    #[state(skip)]
    is_edit: bool,
    /// Existing variable names (for duplicate checking). Excludes the current
    /// variable's name in edit mode.
    #[state(skip)]
    existing_names: Vec<String>,
    name: String,
    /// Inline type list (same pattern as TypePickerModal).
    types: ListState<FieldTypeEntry>,
    error: Option<String>,
}

impl AddVariableModal {
    /// Create a new add variable modal.
    pub fn new_modal(client: DataverseClient, existing_names: Vec<String>) -> Self {
        let default_entry = FieldTypeEntry(FieldType::Simple(AttributeType::String));
        Self::new(
            client,
            false,
            existing_names,
            String::new(),
            ListState::new(vec![default_entry]),
            None,
        )
    }

    /// Create an edit variable modal with initial name and type.
    pub fn edit_modal(
        client: DataverseClient,
        name: &str,
        declared_type: ValueType,
        existing_names: Vec<String>,
    ) -> Self {
        // Exclude the current name so renaming to the same name is allowed
        let existing_names = existing_names
            .into_iter()
            .filter(|n| n != name)
            .collect();
        let entries: Vec<FieldTypeEntry> = Self::value_type_to_entries(declared_type);
        Self::new(
            client,
            true,
            existing_names,
            name.to_string(),
            ListState::new(entries),
            None,
        )
    }

    /// Convert `ValueType` into list entries.
    fn value_type_to_entries(vt: ValueType) -> Vec<FieldTypeEntry> {
        match vt {
            ValueType::Known(ft) => vec![FieldTypeEntry(ft)],
            ValueType::Union(types) => types.into_iter().map(FieldTypeEntry).collect(),
            _ => vec![],
        }
    }

    /// Convert list entries into `ValueType`.
    fn entries_to_value_type(entries: &[FieldTypeEntry]) -> ValueType {
        match entries.len() {
            0 => ValueType::Known(FieldType::Simple(AttributeType::String)),
            1 => ValueType::Known(entries[0].0.clone()),
            _ => ValueType::Union(entries.iter().map(|e| e.0.clone()).collect()),
        }
    }

    fn type_count(&self) -> usize {
        self.types.with_ref(|s| s.items.len())
    }

    fn has_focused_type(&self) -> bool {
        self.types.with_ref(|s| s.focused_key.is_some())
    }
}

#[modal_impl]
impl AddVariableModal {
    fn default_result(&self) -> Option<AddVariableResult> {
        None
    }

    #[on_start]
    async fn on_start(&self, mx: &ModalContext<Option<AddVariableResult>>) {
        mx.focus("variable-name-input");
    }

    #[keybinds]
    fn keybinds() {
        bind("escape", cancel);
        bind("a", add_type);
        bind("d", remove_type);
    }

    #[handler]
    async fn cancel(&self, mx: &ModalContext<Option<AddVariableResult>>) {
        mx.close(None);
    }

    #[handler]
    async fn add_type(&self, gx: &GlobalContext) {
        // Step 1: Pick category
        let Some(category) = gx.modal(TypeCategoryPickerModal::new_modal()).await else {
            return;
        };

        // Step 2: Dispatch to sub-picker
        let field_type = match category {
            TypeCategory::Simple => gx.modal(SimpleTypePickerModal::new_modal()).await,

            TypeCategory::Lookup => {
                let client = self.client.clone();
                let result = gx
                    .modal(LoadingModal::run_with_default(
                        "Loading entities...",
                        || Err(dataverse_lib::error::Error::Cancelled),
                        async move {
                            client.metadata().all_entities().await.map(|entities| {
                                let mut names: Vec<String> =
                                    entities.into_iter().map(|e| e.logical_name).collect();
                                names.sort();
                                names
                            })
                        },
                    ))
                    .await;

                match result {
                    Ok(entities) => {
                        gx.modal(LookupTypePickerModal::new_modal(entities)).await
                    }
                    Err(e) if e.is_cancelled() => return,
                    Err(e) => {
                        log::error!("Failed to fetch entities: {}", e);
                        gx.toast(Toast::error("Failed to fetch entity list"));
                        return;
                    }
                }
            }

            TypeCategory::OptionSet => {
                let client = self.client.clone();
                let result = gx
                    .modal(LoadingModal::run_with_default(
                        "Loading option sets...",
                        || Err(dataverse_lib::error::Error::Cancelled),
                        async move {
                            client
                                .metadata()
                                .all_global_option_sets()
                                .await
                                .map(|option_sets| {
                                    let mut names: Vec<String> =
                                        option_sets.into_iter().map(|os| os.name).collect();
                                    names.sort();
                                    names
                                })
                        },
                    ))
                    .await;

                match result {
                    Ok(names) => {
                        gx.modal(OptionSetTypePickerModal::new_modal(names)).await
                    }
                    Err(e) if e.is_cancelled() => return,
                    Err(e) => {
                        log::error!("Failed to fetch option sets: {}", e);
                        gx.toast(Toast::error("Failed to fetch option set list"));
                        return;
                    }
                }
            }
        };

        // Step 3: Add to list (skip duplicates)
        if let Some(ft) = field_type {
            let entry = FieldTypeEntry(ft);
            let already_exists = self
                .types
                .with_ref(|s| s.items.iter().any(|e| e.key() == entry.key()));

            if already_exists {
                gx.toast(Toast::warning("Type already in list"));
            } else {
                self.types.update(|s| {
                    s.push_item(entry);
                });
            }
        }
    }

    #[handler]
    async fn remove_type(&self, _cx: &AppContext) {
        let focused_key = self.types.with_ref(|s| s.focused_key.clone());

        if let Some(key) = focused_key {
            self.types.update(|s| {
                let new_items: Vec<_> = s
                    .items
                    .iter()
                    .filter(|e| e.key() != key)
                    .cloned()
                    .collect();
                s.set_items(new_items);
            });
        }
    }

    #[handler]
    async fn submit(&self, mx: &ModalContext<Option<AddVariableResult>>) {
        let name = self.name.get().trim().to_string();
        if name.is_empty() {
            self.error.set(Some("Name is required".to_string()));
            return;
        }

        if name.contains(' ') {
            self.error
                .set(Some("Variable name cannot contain spaces".to_string()));
            return;
        }

        // Remove leading $ if user typed it
        let name = name.strip_prefix('$').unwrap_or(&name).to_string();

        // Check for duplicate names
        if self.existing_names.iter().any(|n| n == &name) {
            self.error
                .set(Some(format!("Variable '{}' already exists", name)));
            return;
        }

        let types = self.types.with_ref(|s| s.items.clone());
        if types.is_empty() {
            self.error
                .set(Some("At least one type is required".to_string()));
            return;
        }

        let declared_type = Self::entries_to_value_type(&types);

        mx.close(Some(AddVariableResult {
            name,
            declared_type,
        }));
    }

    fn element(&self) -> Element {
        let error = self.error.get();
        let type_count = self.type_count();
        let has_focused = self.has_focused_type();
        let title = if self.is_edit {
            "Edit Variable"
        } else {
            "Add Variable"
        };
        let submit_label = if self.is_edit { "Save" } else { "Add" };

        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                text (content: {title}) style (bold, fg: interact)

                if let Some(err) = error {
                    text (content: {err}) style (fg: error)
                }

                text (content: "Name") style (fg: muted)
                input (state: self.name, id: "variable-name-input", placeholder: "variable_name")
                    on_submit: submit()
                text (content: "Will be accessible as $name in transforms") style (fg: muted)

                text (content: {format!("Types ({})", type_count)}) style (fg: muted)

                box_ (height: fill, width: fill) style (bg: surface2) {
                    if type_count == 0 {
                        column (width: fill, height: fill, justify: center, align: center) {
                            text (content: "No types added") style (fg: muted)
                            text (content: "Press 'a' to add a type") style (fg: muted)
                        }
                    }
                    if type_count > 0 {
                        list (state: self.types, id: "types-list", width: fill, height: fill)
                    }
                }

                row (width: fill, justify: between) {
                    button (label: "Cancel", hint: "esc", id: "cancel-btn")
                        on_activate: cancel()

                    row (gap: 1) {
                        button (
                            label: "Remove",
                            hint: "d",
                            id: "remove-btn",
                            disabled: {!has_focused}
                        )
                            on_activate: remove_type()

                        button (label: "Add Type", hint: "a", id: "add-type-btn")
                            on_activate: add_type()

                        button (label: {submit_label}, id: "submit-btn", disabled: {type_count == 0})
                            on_activate: submit()
                    }
                }
            }
        }
    }
}
