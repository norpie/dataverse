//! Sort field editor modal for adding/editing order by entries.

use dataverse_lib::DataverseClient;
use dataverse_lib::api::query::Direction;
use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{Autocomplete, AutocompleteState, Button, RadioGroup, RadioState, Text};

/// Modal for selecting a sort field and direction.
#[modal]
pub struct SortFieldEditorModal {
    #[state(skip)]
    client: Option<DataverseClient>,
    #[state(skip)]
    entity: String,

    field: AutocompleteState<String>,
    direction: RadioState<String>,
    loading: bool,
    error: Option<String>,
}

impl SortFieldEditorModal {
    pub fn new(client: DataverseClient, entity: impl Into<String>) -> Self {
        Self {
            client: Some(client),
            entity: entity.into(),
            ..Default::default()
        }
    }
}

#[modal_impl]
impl SortFieldEditorModal {
    fn default_result(&self) -> Option<(String, Direction)> {
        None
    }

    #[on_start]
    async fn on_start(&self, mx: &ModalContext<Option<(String, Direction)>>) {
        self.loading.set(true);

        // Initialize direction radio
        self.direction.set(
            RadioState::new([
                ("asc".to_string(), "Ascending".to_string()),
                ("desc".to_string(), "Descending".to_string()),
            ])
            .with_value("asc".to_string()),
        );

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
                self.field.set(AutocompleteState::new(options));
                self.loading.set(false);
                mx.focus("sort-field-autocomplete");
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
    async fn cancel(&self, mx: &ModalContext<Option<(String, Direction)>>) {
        mx.close(None);
    }

    #[handler]
    async fn confirm(&self, mx: &ModalContext<Option<(String, Direction)>>) {
        let field = self.field.with_ref(|s| s.value().cloned());
        let Some(field_name) = field else {
            return;
        };

        let direction = self.direction.with_ref(|s| match s.value.as_deref() {
            Some("desc") => Direction::Desc,
            _ => Direction::Asc,
        });

        mx.close(Some((field_name, direction)));
    }

    fn element(&self) -> Element {
        let loading = self.loading.get();
        let error = self.error.get();

        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                text (content: "Add Sort Field") style (bold, fg: interact)

                if let Some(err) = error {
                    text (content: {err}) style (fg: primary)
                }

                if loading {
                    text (content: "Loading attributes...") style (fg: muted)
                } else {
                    text (content: "Field") style (fg: muted)
                    autocomplete (state: self.field, id: "sort-field-autocomplete", placeholder: "Search fields...")
                    text (content: "Direction") style (fg: muted)
                    radio_group (state: self.direction, id: "sort-direction")
                }

                row (width: fill, justify: between) {
                    button (label: "Cancel", hint: "esc", id: "cancel") on_activate: cancel()
                    button (label: "Ok", id: "ok") on_activate: confirm()
                }
            }
        }
    }
}
