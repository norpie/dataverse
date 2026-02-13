//! Modal for selecting a target field path with autocomplete and validation.
//!
//! Used by both find conditions and match conditions. Supports dotted paths
//! through lookup navigation (e.g., `contact.emailaddress1`).

use dataverse_lib::DataverseClient;
use log::debug;
use rafter::page;
use rafter::prelude::*;
use rafter::widgets::AutocompleteState;
use tuidom::Element;

use super::path_suggestions::PathSuggestionGenerator;
use crate::apps::migration::validation::PathValidator;
use crate::apps::migration::validation::ValidationContext;
use crate::apps::migration::validation::ValidationResult;
use crate::widgets::Spinner;

/// Modal for selecting a target field path with autocomplete and validation.
///
/// Supports both simple fields (`emailaddress1`) and dotted paths through
/// lookups (`contact.emailaddress1`, `ownerid[systemuser].fullname`).
#[modal(size = Sm)]
pub struct TargetFieldModal {
    /// The Dataverse client for metadata lookups.
    #[state(skip)]
    client: DataverseClient,
    /// The entity to resolve fields against (find entity or target entity).
    #[state(skip)]
    entity: String,
    /// Modal title.
    #[state(skip)]
    title: String,
    /// Description text shown below the title.
    #[state(skip)]
    description: String,

    /// Autocomplete state for path input.
    autocomplete: AutocompleteState<String>,
    /// Current validation result.
    validation: ValidationResult,
}

impl TargetFieldModal {
    /// Create a new target field modal for adding a condition.
    pub fn new_modal(
        client: DataverseClient,
        entity: String,
        title: &str,
        description: &str,
    ) -> Self {
        let autocomplete = AutocompleteState::new(Vec::<(String, String)>::new());

        Self::new(
            client,
            entity,
            title.to_string(),
            description.to_string(),
            autocomplete,
            ValidationResult::Loading,
        )
    }

    /// Create a target field modal for editing an existing condition.
    pub fn edit_modal(
        client: DataverseClient,
        entity: String,
        title: &str,
        description: &str,
        current_field: &str,
    ) -> Self {
        let mut autocomplete = AutocompleteState::new(Vec::<(String, String)>::new());
        autocomplete.text = current_field.to_string();
        autocomplete.cursor = autocomplete.text.len();

        Self::new(
            client,
            entity,
            title.to_string(),
            description.to_string(),
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
impl TargetFieldModal {
    fn default_result(&self) -> Option<String> {
        None
    }

    #[on_start]
    async fn on_start(&self, mx: &ModalContext<Option<String>>) {
        let path = self.current_path();
        let generator = PathSuggestionGenerator::new(
            self.client.clone(),
            self.client.clone(),
            self.entity.clone(),
            Vec::new(),
        );
        let suggestions = generator.generate_suggestions(&path).await;

        self.autocomplete.update(|s| {
            s.options = suggestions;
            s.refilter();
        });

        mx.focus("target-field-input");

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
        debug!("[TargetField] on_path_change triggered, path: {:?}", path);

        let generator = PathSuggestionGenerator::new(
            self.client.clone(),
            self.client.clone(),
            self.entity.clone(),
            Vec::new(),
        );
        let suggestions = generator.generate_suggestions(&path).await;
        debug!("[TargetField] Generated {} suggestions", suggestions.len());

        self.autocomplete.update(|s| {
            s.options = suggestions;
            s.refilter();
        });

        self.validate_path().await;
    }

    async fn validate_path(&self) {
        let path = self.current_path();

        if path.trim().is_empty() {
            self.validation.set(ValidationResult::Invalid(
                "Path cannot be empty".to_string(),
            ));
            return;
        }

        self.validation.set(ValidationResult::Loading);

        // Validate as a field path rooted on the target entity.
        // We reuse PathValidator with the entity as "source_entity" and no variables,
        // since target-side paths are plain field paths only.
        let validator = PathValidator::new(self.client.clone(), self.client.clone());
        let ctx = ValidationContext {
            source_entity: self.entity.clone(),
            variable_types: Default::default(),
        };

        let result = validator.validate(&path, &ctx).await;
        self.validation.set(result);
    }

    fn element(&self) -> Element {
        let validation = self.validation.get();
        let is_valid = self.is_valid();
        let is_loading = self.is_loading();
        let title = self.title.clone();
        let description = self.description.clone();

        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                text (content: {&title}) style (bold, fg: interact)

                text (content: {&description}) style (fg: muted)

                column (gap: 0, width: fill) {
                    text (content: "Target Field") style (fg: muted)
                    autocomplete (
                        state: self.autocomplete,
                        id: "target-field-input",
                        placeholder: "e.g., emailaddress1, contact.fullname",
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
