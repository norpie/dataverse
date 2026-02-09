//! Modal for creating/editing a Find condition's target field.

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::AutocompleteState;
use rafter::widgets::Button;
use rafter::widgets::Text;
use tuidom::Element;

/// Modal for selecting the target field of a find condition.
#[modal(size = Sm)]
pub struct FindConditionModal {
    /// Autocomplete state for target field selection.
    target_field: AutocompleteState<String>,
    /// Error message.
    error: Option<String>,
}

impl FindConditionModal {
    /// Create a new find condition modal.
    ///
    /// `field_options` is a list of (logical_name, display_label) tuples.
    pub fn new_modal(field_options: Vec<(String, String)>) -> Self {
        Self::new(AutocompleteState::new(field_options), None)
    }

    /// Create a find condition modal for editing an existing condition.
    pub fn edit_modal(field_options: Vec<(String, String)>, current_field: &str) -> Self {
        let state = AutocompleteState::new(field_options).with_value(current_field.to_string());
        Self::new(state, None)
    }
}

#[modal_impl]
impl FindConditionModal {
    fn default_result(&self) -> Option<String> {
        None
    }

    #[on_start]
    async fn on_start(&self, mx: &ModalContext<Option<String>>) {
        mx.focus("target-field-input");
    }

    #[keybinds]
    fn keybinds() {
        bind("escape", cancel);
    }

    #[handler]
    async fn cancel(&self, mx: &ModalContext<Option<String>>) {
        mx.close(None);
    }

    #[handler]
    async fn submit(&self, mx: &ModalContext<Option<String>>) {
        let target_field = self.target_field.with_ref(|s| s.value().cloned());

        let Some(target_field) = target_field else {
            self.error
                .set(Some("Please select a target field".to_string()));
            return;
        };

        mx.close(Some(target_field));
    }

    fn element(&self) -> Element {
        let error = self.error.get();

        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                text (content: "Find Condition") style (bold, fg: interact)

                text (content: "Select the field to match on in the target entity.") style (fg: muted)

                if let Some(err) = error {
                    text (content: {err}) style (fg: error)
                }

                column (gap: 0, width: fill, height: fill) {
                    text (content: "Target Field") style (fg: muted)
                    autocomplete (
                        state: self.target_field,
                        id: "target-field-input",
                        placeholder: "Select field..."
                    )
                        on_submit: submit()
                }

                row (width: fill, justify: between) {
                    button (label: "Cancel", hint: "esc", id: "cancel-btn") on_activate: cancel()
                    button (label: "Save", id: "save-btn") on_activate: submit()
                }
            }
        }
    }
}
