//! Main type picker modal for selecting one or more `FieldType` values.
//!
//! Orchestrates the category picker and sub-pickers (simple, lookup, option set).
//! Returns `Vec<FieldType>` — the caller converts: 1 item → `Known(ft)`, 2+ → `Union(vec)`.

use dataverse_lib::DataverseClient;
use dataverse_lib::model::FieldType;
use rafter::page;
use rafter::prelude::*;
use rafter::widgets::Button;
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

/// Wrapper around `FieldType` to implement `ListItem`.
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

/// Modal for picking one or more field types.
///
/// Displays a list of currently selected types with add/remove controls.
/// The "Add" flow chains through category picker → sub-picker.
/// Metadata fetches (entities, option sets) use the provided client.
///
/// Returns `Some(Vec<FieldType>)` on save (may be empty), `None` on cancel.
#[modal(size = Md)]
pub struct TypePickerModal {
    #[state(skip)]
    client: DataverseClient,
    types: ListState<FieldTypeEntry>,
}

impl TypePickerModal {
    /// Create a new type picker modal with no initial types.
    pub fn new_modal(client: DataverseClient) -> Self {
        Self::new(client, ListState::new(vec![]))
    }

    /// Create a type picker modal with initial types (for editing).
    pub fn with_types(client: DataverseClient, initial: Vec<FieldType>) -> Self {
        let entries: Vec<FieldTypeEntry> = initial.into_iter().map(FieldTypeEntry).collect();
        Self::new(client, ListState::new(entries))
    }

    fn has_focused(&self) -> bool {
        self.types.with_ref(|s| s.focused_key.is_some())
    }

    fn type_count(&self) -> usize {
        self.types.with_ref(|s| s.items.len())
    }

    fn current_types(&self) -> Vec<FieldType> {
        self.types
            .with_ref(|s| s.items.iter().map(|e| e.0.clone()).collect())
    }
}

#[modal_impl]
impl TypePickerModal {
    fn default_result(&self) -> Option<Vec<FieldType>> {
        None
    }

    #[on_start]
    async fn on_start(&self, mx: &ModalContext<Option<Vec<FieldType>>>) {
        mx.focus("types-list");
    }

    #[keybinds]
    fn keybinds() {
        bind("escape", cancel);
        bind("a", add_type);
        bind("d", remove_type);
    }

    #[handler]
    async fn cancel(&self, mx: &ModalContext<Option<Vec<FieldType>>>) {
        mx.close(None);
    }

    #[handler]
    async fn submit(&self, mx: &ModalContext<Option<Vec<FieldType>>>) {
        mx.close(Some(self.current_types()));
    }

    #[handler]
    async fn add_type(&self, gx: &GlobalContext) {
        // Step 1: Pick category
        let Some(category) = gx.modal(TypeCategoryPickerModal::new_modal()).await else {
            return;
        };

        // Step 2: Dispatch to sub-picker based on category
        let field_type =
            match category {
                TypeCategory::Simple => gx.modal(SimpleTypePickerModal::new_modal()).await,

                TypeCategory::Lookup => {
                    // Fetch entity names first
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
                        Ok(entities) => gx.modal(LookupTypePickerModal::new_modal(entities)).await,
                        Err(e) if e.is_cancelled() => return,
                        Err(e) => {
                            log::error!("Failed to fetch entities: {}", e);
                            gx.toast(Toast::error("Failed to fetch entity list"));
                            return;
                        }
                    }
                }

                TypeCategory::OptionSet => {
                    // Fetch global option set names first
                    let client = self.client.clone();
                    let result = gx
                        .modal(LoadingModal::run_with_default(
                            "Loading option sets...",
                            || Err(dataverse_lib::error::Error::Cancelled),
                            async move {
                                client.metadata().all_global_option_sets().await.map(
                                    |option_sets| {
                                        let mut names: Vec<String> =
                                            option_sets.into_iter().map(|os| os.name).collect();
                                        names.sort();
                                        names
                                    },
                                )
                            },
                        ))
                        .await;

                    match result {
                        Ok(names) => gx.modal(OptionSetTypePickerModal::new_modal(names)).await,
                        Err(e) if e.is_cancelled() => return,
                        Err(e) => {
                            log::error!("Failed to fetch option sets: {}", e);
                            gx.toast(Toast::error("Failed to fetch option set list"));
                            return;
                        }
                    }
                }
            };

        // Step 3: Add to list if a type was selected (skip duplicates)
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
                let new_items: Vec<_> =
                    s.items.iter().filter(|e| e.key() != key).cloned().collect();
                s.set_items(new_items);
            });
        }
    }

    fn element(&self) -> Element {
        let count = self.type_count();
        let has_focused = self.has_focused();

        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                text (content: "Select Types") style (bold, fg: interact)

                text (content: {format!("Types ({})", count)}) style (fg: muted)

                box_ (height: fill, width: fill) style (bg: surface2) {
                    if count == 0 {
                        column (width: fill, height: fill, justify: center, align: center) {
                            text (content: "No types added yet") style (fg: muted)
                            text (content: "Press 'a' to add a type") style (fg: muted)
                        }
                    }
                    if count > 0 {
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

                        button (label: "Add", hint: "a", id: "add-btn")
                            on_activate: add_type()

                        button (label: "Save", id: "save-btn", disabled: {count == 0})
                            on_activate: submit()
                    }
                }
            }
        }
    }
}
