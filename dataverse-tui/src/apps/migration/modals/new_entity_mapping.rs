//! Modal for creating a new entity mapping.

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::Autocomplete;
use rafter::widgets::AutocompleteState;
use rafter::widgets::Button;
use rafter::widgets::Text;
use tuidom::Element;

/// Result of the new entity mapping modal.
#[derive(Debug, Clone)]
pub struct NewEntityMappingResult {
    pub source_entity: String,
    pub target_entity: String,
}

/// Modal for creating a new entity mapping.
#[modal(size = Md)]
pub struct NewEntityMappingModal {
    source_entity: AutocompleteState<String>,
    target_entity: AutocompleteState<String>,
    error: Option<String>,
}

impl NewEntityMappingModal {
    /// Create a new entity mapping modal with entity options.
    pub fn with_entities(source_entities: Vec<String>, target_entities: Vec<String>) -> Self {
        // AutocompleteState expects (key, label) pairs - use logical_name for both
        let source_options: Vec<_> = source_entities
            .into_iter()
            .map(|name| (name.clone(), name))
            .collect();
        let target_options: Vec<_> = target_entities
            .into_iter()
            .map(|name| (name.clone(), name))
            .collect();

        Self::new(
            AutocompleteState::new(source_options),
            AutocompleteState::new(target_options),
            None,
        )
    }
}

#[modal_impl]
impl NewEntityMappingModal {
    fn default_result(&self) -> Option<NewEntityMappingResult> {
        None
    }

    #[on_start]
    async fn on_start(&self, mx: &ModalContext<Option<NewEntityMappingResult>>) {
        mx.focus("source-entity");
    }

    #[keybinds]
    fn keybinds() {
        bind("escape", cancel);
    }

    #[handler]
    async fn cancel(&self, mx: &ModalContext<Option<NewEntityMappingResult>>) {
        mx.close(None);
    }

    #[handler]
    async fn submit(&self, mx: &ModalContext<Option<NewEntityMappingResult>>) {
        let source = self.source_entity.with_ref(|s| s.value().cloned());
        let target = self.target_entity.with_ref(|s| s.value().cloned());

        let (Some(source), Some(target)) = (source, target) else {
            self.error
                .set(Some("Please select both source and target entities".to_string()));
            return;
        };

        mx.close(Some(NewEntityMappingResult {
            source_entity: source,
            target_entity: target,
        }));
    }

    fn element(&self) -> Element {
        let error = self.error.get();

        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                text (content: "New Entity Mapping") style (bold, fg: interact)

                if let Some(err) = error {
                    text (content: {err}) style (fg: error)
                }

                column (gap: 1, width: fill, height: fill) {
                    text (content: "Source Entity") style (fg: muted)
                    autocomplete (state: self.source_entity, id: "source-entity", placeholder: "Select source entity...")

                    text (content: "Target Entity") style (fg: muted)
                    autocomplete (state: self.target_entity, id: "target-entity", placeholder: "Select target entity...")
                }

                row (width: fill, justify: between) {
                    button (label: "Cancel", hint: "esc", id: "cancel-btn") on_activate: cancel()
                    button (label: "Create", id: "create-btn") on_activate: submit()
                }
            }
        }
    }
}
