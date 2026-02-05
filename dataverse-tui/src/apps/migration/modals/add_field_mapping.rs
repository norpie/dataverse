//! Modal for adding a new field mapping.

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::AutocompleteState;
use rafter::widgets::Button;
use rafter::widgets::Text;
use tuidom::Element;

/// Result of the add field mapping modal.
#[derive(Debug, Clone)]
pub struct AddFieldMappingResult {
    pub target_field: String,
}

/// Modal for adding a new field mapping.
#[modal(size = Sm)]
pub struct AddFieldMappingModal {
    /// Autocomplete state for target field selection.
    target_field: AutocompleteState<String>,
    /// Error message.
    error: Option<String>,
}

impl AddFieldMappingModal {
    /// Create a new add field mapping modal.
    ///
    /// `target_fields` is a list of (logical_name, display_name) tuples.
    pub fn new_modal(target_fields: Vec<(String, String)>) -> Self {
        Self::new(AutocompleteState::new(target_fields), None)
    }

    /// Create an edit field mapping modal with initial value.
    pub fn edit_modal(target_fields: Vec<(String, String)>, current_field: &str) -> Self {
        let state = AutocompleteState::new(target_fields).with_value(current_field.to_string());
        Self::new(state, None)
    }
}

#[modal_impl]
impl AddFieldMappingModal {
    fn default_result(&self) -> Option<AddFieldMappingResult> {
        None
    }

    #[on_start]
    async fn on_start(&self, mx: &ModalContext<Option<AddFieldMappingResult>>) {
        mx.focus("target-field-input");
    }

    #[keybinds]
    fn keybinds() {
        bind("escape", cancel);
    }

    #[handler]
    async fn cancel(&self, mx: &ModalContext<Option<AddFieldMappingResult>>) {
        mx.close(None);
    }

    #[handler]
    async fn submit(&self, mx: &ModalContext<Option<AddFieldMappingResult>>) {
        let target_field = self.target_field.with_ref(|s| s.value().cloned());

        let Some(target_field) = target_field else {
            self.error
                .set(Some("Please select a target field".to_string()));
            return;
        };

        mx.close(Some(AddFieldMappingResult { target_field }));
    }

    fn element(&self) -> Element {
        let error = self.error.get();

        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                text (content: "Add Field Mapping") style (bold, fg: interact)

                if let Some(err) = error {
                    text (content: {err}) style (fg: error)
                }

                column (gap: 1, width: fill, height: fill) {
                    text (content: "Target Field") style (fg: muted)
                    autocomplete (
                        state: self.target_field,
                        id: "target-field-input",
                        placeholder: "Select target field..."
                    )
                }

                row (width: fill, justify: between) {
                    button (label: "Cancel", hint: "esc", id: "cancel-btn") on_activate: cancel()
                    button (label: "Add", id: "add-btn") on_activate: submit()
                }
            }
        }
    }
}
