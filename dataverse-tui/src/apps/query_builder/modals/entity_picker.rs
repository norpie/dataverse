//! Entity picker modal for selecting a Dataverse entity.

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{Autocomplete, AutocompleteState, Button, Text};

/// Modal for selecting an entity set name.
#[modal(size = Md)]
pub struct EntityPickerModal {
    #[state(skip)]
    options: Vec<(String, String)>,

    entities: AutocompleteState<String>,
}

impl EntityPickerModal {
    /// Create with pre-fetched entity options: (entity_set_name, display_label).
    pub fn new(options: Vec<(String, String)>) -> Self {
        Self {
            options,
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
        self.entities
            .set(AutocompleteState::new(self.options.clone()));
        mx.focus("entity-autocomplete");
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
    async fn on_select(&self, mx: &ModalContext<Option<String>>) {
        let selected = self.entities.with_ref(|s| s.value().cloned());
        if selected.is_some() {
            mx.close(selected);
        }
    }

    fn element(&self) -> Element {
        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                text (content: "Select Entity") style (bold, fg: interact)
                autocomplete (state: self.entities, id: "entity-autocomplete", placeholder: "Search entities...")
                    on_select: on_select()
                row (width: fill, justify: between) {
                    button (label: "Cancel", hint: "esc", id: "cancel") on_activate: cancel()
                    button (label: "Ok", id: "ok") on_activate: on_select()
                }
            }
        }
    }
}
