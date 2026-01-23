//! Field picker modal for selecting entity attributes.

use dataverse_lib::DataverseClient;
use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{Autocomplete, AutocompleteState, Button, SelectionMode, Text};

/// Modal for selecting one or more entity fields.
#[modal(size = Md)]
pub struct FieldPickerModal {
    #[state(skip)]
    client: Option<DataverseClient>,
    #[state(skip)]
    entity: String,

    fields: AutocompleteState<String>,
    loading: bool,
    error: Option<String>,
}

impl FieldPickerModal {
    pub fn new(client: DataverseClient, entity: impl Into<String>) -> Self {
        Self {
            client: Some(client),
            entity: entity.into(),
            ..Default::default()
        }
    }
}

#[modal_impl]
impl FieldPickerModal {
    fn default_result(&self) -> Vec<String> {
        vec![]
    }

    #[on_start]
    async fn on_start(&self, mx: &ModalContext<Vec<String>>) {
        self.loading.set(true);

        let Some(client) = &self.client else {
            self.error.set(Some("No client available".to_string()));
            self.loading.set(false);
            return;
        };

        let result = client.metadata().attributes(&self.entity).await;

        match result {
            Ok(attrs) => {
                let options: Vec<(String, String)> = attrs
                    .iter()
                    .filter(|a| a.is_valid_for_read && a.attribute_of.is_none())
                    .map(|a| {
                        let display = a
                            .display_name
                            .text()
                            .map(|d| format!("{} ({})", d, a.logical_name))
                            .unwrap_or_else(|| a.logical_name.clone());
                        (a.logical_name.clone(), display)
                    })
                    .collect();
                self.fields
                    .set(AutocompleteState::new(options).with_selection(SelectionMode::Multi));
                self.loading.set(false);
                mx.focus("field-autocomplete");
            }
            Err(e) => {
                self.error
                    .set(Some(format!("Failed to load attributes: {}", e)));
                self.loading.set(false);
            }
        }
    }

    #[keybinds]
    fn keys() {
        bind("escape", cancel);
    }

    #[handler]
    async fn cancel(&self, mx: &ModalContext<Vec<String>>) {
        mx.close(vec![]);
    }

    #[handler]
    async fn confirm(&self, mx: &ModalContext<Vec<String>>) {
        let selected: Vec<String> = self
            .fields
            .with_ref(|s| s.selected_values().cloned().collect());
        mx.close(selected);
    }

    fn element(&self) -> Element {
        let loading = self.loading.get();
        let error = self.error.get();
        let count = self.fields.with_ref(|s| s.selection.selected.len());

        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                text (content: "Select Fields") style (bold, fg: interact)

                if let Some(err) = error {
                    text (content: {err}) style (fg: primary)
                }

                if loading {
                    text (content: "Loading attributes...") style (fg: muted)
                } else {
                    autocomplete (state: self.fields, id: "field-autocomplete", placeholder: "Search fields...")
                    text (content: {format!("{} selected", count)}) style (fg: muted)
                }

                row (width: fill, justify: between) {
                    button (label: "Cancel", hint: "esc", id: "cancel") on_activate: cancel()
                    button (label: "Ok", id: "ok") on_activate: confirm()
                }
            }
        }
    }
}
