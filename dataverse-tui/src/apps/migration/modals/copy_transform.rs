//! Modal for editing a Copy transform.

use dataverse_lib::DataverseClient;
use log::debug;
use rafter::page;
use rafter::prelude::*;
use rafter::widgets::AutocompleteState;
use tuidom::Element;

use super::path_suggestions::PathSuggestionGenerator;
use super::path_suggestions::VariableInfo;
use crate::apps::migration::validation::PathValidator;
use crate::apps::migration::validation::ValidationContext;
use crate::apps::migration::validation::ValidationResult;
use crate::widgets::Spinner;

/// Modal for editing a Copy transform's path.
#[modal(size = Sm)]
pub struct CopyTransformModal {
    /// The Dataverse client for metadata lookups.
    #[state(skip)]
    client: DataverseClient,
    /// The source entity logical name.
    #[state(skip)]
    source_entity: String,
    /// Available variables with type info.
    #[state(skip)]
    variables: Vec<VariableInfo>,

    /// Autocomplete state for path input.
    autocomplete: AutocompleteState<String>,
    /// Current validation result.
    validation: ValidationResult,
}

impl CopyTransformModal {
    /// Create a new Copy transform modal.
    pub fn new_modal(
        client: DataverseClient,
        source_entity: String,
        variables: Vec<VariableInfo>,
        current_path: String,
    ) -> Self {
        // Initialize autocomplete with empty options (will populate in on_start)
        let mut autocomplete = AutocompleteState::new(Vec::<(String, String)>::new());
        autocomplete.text = current_path;
        autocomplete.cursor = autocomplete.text.len();

        Self::new(
            client,
            source_entity,
            variables,
            autocomplete,
            ValidationResult::Loading,
        )
    }

    fn is_valid(&self) -> bool {
        matches!(self.validation.get(), ValidationResult::Valid(_))
    }

    fn is_loading(&self) -> bool {
        matches!(self.validation.get(), ValidationResult::Loading)
    }

    fn current_path(&self) -> String {
        self.autocomplete.get().text.clone()
    }
}

#[modal_impl]
impl CopyTransformModal {
    fn default_result(&self) -> Option<String> {
        None
    }

    #[on_start]
    async fn on_start(&self, mx: &ModalContext<Option<String>>) {
        // Generate initial suggestions
        let path = self.current_path();
        let generator = PathSuggestionGenerator::new(
            self.client.clone(),
            self.source_entity.clone(),
            self.variables.clone(),
        );
        let suggestions = generator.generate_suggestions(&path).await;

        self.autocomplete.update(|s| {
            s.options = suggestions;
            s.refilter();
        });

        mx.focus("path-autocomplete");

        // Validate initial path
        self.validate_path().await;
    }

    #[keybinds]
    fn keybinds() {
        bind("escape", cancel);
        bind("ctrl+s", save);
    }

    #[handler]
    async fn cancel(&self, mx: &ModalContext<Option<String>>) {
        mx.close(None);
    }

    #[handler]
    async fn save(&self, mx: &ModalContext<Option<String>>) {
        if self.is_valid() {
            mx.close(Some(self.current_path()));
        }
    }

    #[handler]
    async fn on_path_change(&self, _cx: &AppContext) {
        let path = self.current_path();
        debug!("[CopyTransform] on_path_change triggered, path: {:?}", path);

        // Generate new suggestions based on current input
        let generator = PathSuggestionGenerator::new(
            self.client.clone(),
            self.source_entity.clone(),
            self.variables.clone(),
        );
        let suggestions = generator.generate_suggestions(&path).await;
        debug!(
            "[CopyTransform] Generated {} suggestions, updating autocomplete",
            suggestions.len()
        );

        self.autocomplete.update(|s| {
            s.options = suggestions;
            s.refilter();
        });

        // Validate
        self.validate_path().await;
    }

    async fn validate_path(&self) {
        let path = self.current_path();

        // Empty path is invalid
        if path.trim().is_empty() {
            self.validation.set(ValidationResult::Invalid(
                "Path cannot be empty".to_string(),
            ));
            return;
        }

        // Set loading state
        self.validation.set(ValidationResult::Loading);

        // Create validator and context
        let validator = PathValidator::new(self.client.clone());
        let ctx = ValidationContext {
            source_entity: self.source_entity.clone(),
            variable_types: self
                .variables
                .iter()
                .map(|v| (v.name.clone(), v.declared_type.clone()))
                .collect(),
        };

        // Validate
        let result = validator.validate(&path, &ctx).await;
        self.validation.set(result);
    }

    fn element(&self) -> Element {
        let validation = self.validation.get();
        let is_valid = self.is_valid();
        let is_loading = self.is_loading();

        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                text (content: "Edit Copy Transform") style (bold, fg: interact)

                column (gap: 0, width: fill) {
                    text (content: "Path") style (fg: muted)
                    autocomplete (
                        state: self.autocomplete,
                        id: "path-autocomplete",
                        placeholder: "e.g., name, $variable, #value",
                        width: fill
                    )
                        on_change: on_path_change()
                }

                // Validation status
                box_ (width: fill) {
                    match &validation {
                        ValidationResult::Loading => {
                            row (gap: 1) {
                                spinner ()
                                text (content: "Validating...") style (fg: muted)
                            }
                        }
                        ValidationResult::Valid(valid_path) => {
                            column (gap: 0) {
                                row (gap: 1) {
                                    text (content: "✓") style (fg: success)
                                    text (content: "Valid") style (fg: success)
                                }
                                text (content: {&valid_path.description}) style (fg: muted)
                            }
                        }
                        ValidationResult::Invalid(error) => {
                            row (gap: 1) {
                                text (content: "✗") style (fg: error)
                                text (content: {error}) style (fg: error)
                            }
                        }
                    }
                }

                // Spacer
                box_ (height: fill) {}

                // Buttons
                row (width: fill, justify: between) {
                    button (label: "Cancel", hint: "esc", id: "cancel-btn")
                        on_activate: cancel()
                    button (
                        label: "Save",
                        hint: "ctrl+s",
                        id: "save-btn",
                        disabled: {!is_valid || is_loading}
                    )
                        on_activate: save()
                }
            }
        }
    }
}
