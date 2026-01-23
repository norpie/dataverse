//! Entity picker modal for selecting a Dataverse entity.

use dataverse_lib::DataverseClient;
use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{Autocomplete, AutocompleteState, Button, Text};

/// Modal for selecting an entity set name.
#[modal(size = Md)]
pub struct EntityPickerModal {
    #[state(skip)]
    client: Option<DataverseClient>,

    entities: AutocompleteState<String>,
    loading: bool,
    error: Option<String>,
}

impl EntityPickerModal {
    pub fn new(client: DataverseClient) -> Self {
        Self {
            client: Some(client),
            ..Default::default()
        }
    }
}

#[modal_impl]
impl EntityPickerModal {
    fn default_result(&self) -> Option<String> {
        None
    }

    #[on_start]
    async fn on_start(&self, mx: &ModalContext<Option<String>>) {
        self.loading.set(true);

        let Some(client) = &self.client else {
            self.error.set(Some("No client available".to_string()));
            self.loading.set(false);
            return;
        };

        let result = client.metadata().all_entities().await;

        match result {
            Ok(all) => {
                let options: Vec<(String, String)> = all
                    .iter()
                    .map(|e| {
                        let display = e
                            .display_name
                            .text()
                            .map(|d| format!("{} ({})", d, e.entity_set_name))
                            .unwrap_or_else(|| e.entity_set_name.clone());
                        (e.entity_set_name.clone(), display)
                    })
                    .collect();
                self.entities.set(AutocompleteState::new(options));
                self.loading.set(false);
                mx.focus("entity-autocomplete");
            }
            Err(e) => {
                self.error
                    .set(Some(format!("Failed to load entities: {}", e)));
                self.loading.set(false);
            }
        }
    }

    #[keybinds]
    fn keys() {
        bind("escape", cancel);
    }

    #[handler]
    async fn cancel(&self, mx: &ModalContext<Option<String>>) {
        mx.close(None);
    }

    #[handler]
    async fn confirm(&self, mx: &ModalContext<Option<String>>) {
        let selected = self.entities.with_ref(|s| s.value().cloned());
        if selected.is_some() {
            mx.close(selected);
        }
    }

    #[handler]
    async fn on_select(&self, mx: &ModalContext<Option<String>>) {
        let selected = self.entities.with_ref(|s| s.value().cloned());
        if selected.is_some() {
            mx.close(selected);
        }
    }

    fn element(&self) -> Element {
        let loading = self.loading.get();
        let error = self.error.get();

        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                text (content: "Select Entity") style (bold, fg: interact)

                if let Some(err) = error {
                    text (content: {err}) style (fg: primary)
                }

                if loading {
                    text (content: "Loading entities...") style (fg: muted)
                } else {
                    autocomplete (state: self.entities, id: "entity-autocomplete", placeholder: "Search entities...")
                        on_select: on_select()
                }

                row (width: fill, justify: between) {
                    button (label: "Cancel", hint: "esc", id: "cancel") on_activate: cancel()
                    button (label: "Ok", id: "ok") on_activate: confirm()
                }
            }
        }
    }
}
