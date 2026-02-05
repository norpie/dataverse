//! Modal for editing a Copy transform.

use dataverse_lib::model::metadata::AttributeType;
use dataverse_lib::DataverseClient;
use rafter::page;
use rafter::prelude::*;
use rafter::widgets::Autocomplete;
use rafter::widgets::AutocompleteState;
use rafter::widgets::Button;
use rafter::widgets::Text;
use tuidom::Element;

use crate::apps::migration::validation::parse_path;
use crate::apps::migration::validation::PathExpr;
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
    /// Available variable names (without `$` prefix).
    #[state(skip)]
    variables: Vec<String>,

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
        variables: Vec<String>,
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
        let suggestions = self.generate_suggestions(&path).await;
        
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
        
        // Generate new suggestions based on current input
        let suggestions = self.generate_suggestions(&path).await;
        
        self.autocomplete.update(|s| {
            s.options = suggestions;
            s.refilter();
        });
        
        // Validate
        self.validate_path().await;
    }

    /// Generate autocomplete suggestions based on current input.
    async fn generate_suggestions(&self, input: &str) -> Vec<(String, String)> {
        let input = input.trim();

        // Empty input - show all root options
        if input.is_empty() {
            return self.generate_root_suggestions().await;
        }

        // Check if we're inside brackets (polymorphic target selection)
        if let Some(bracket_pos) = input.rfind('[') {
            if !input[bracket_pos..].contains(']') {
                // Inside brackets - suggest polymorphic targets
                let prefix = &input[..bracket_pos];
                return self.generate_polymorphic_targets(prefix).await;
            }
        }

        // Check if we just typed a dot - suggest fields of target entity
        if input.ends_with('.') {
            return self.generate_field_suggestions_after_dot(input).await;
        }

        // Otherwise show root suggestions (autocomplete fuzzy filters)
        self.generate_root_suggestions().await
    }

    /// Generate root-level suggestions: source fields, variables, system vars.
    async fn generate_root_suggestions(&self) -> Vec<(String, String)> {
        let mut suggestions = Vec::new();

        // Fetch source entity metadata
        if let Ok(metadata) = self.client.metadata().entity(self.source_entity.as_str()).await {
            for attr in &metadata.attributes {
                let type_str = format!("{:?}", attr.attribute_type);
                let label = format!("{} ({})", attr.logical_name, type_str);
                suggestions.push((attr.logical_name.clone(), label));
            }
        }

        // Add variables
        for var in &self.variables {
            let value = format!("${}", var);
            let label = format!("${} (variable)", var);
            suggestions.push((value, label));
        }

        // Add system variables
        for sysvar in &[
            ("value", "current pipeline value"),
            ("type", "value type"),
            ("index", "record index"),
            ("source_entity", "source entity name"),
            ("target_entity", "target entity name"),
        ] {
            let value = format!("#{}", sysvar.0);
            let label = format!("#{} ({})", sysvar.0, sysvar.1);
            suggestions.push((value, label));
        }

        suggestions
    }

    /// Generate field suggestions after a dot (lookup navigation).
    async fn generate_field_suggestions_after_dot(&self, input: &str) -> Vec<(String, String)> {
        // Parse the path up to the dot
        let path_before_dot = &input[..input.len() - 1];
        
        // Try to determine what entity we're on
        let target_entity = match self.resolve_entity_at_path(path_before_dot).await {
            Some(entity) => entity,
            None => return Vec::new(),
        };

        // Fetch fields of target entity
        let mut suggestions = Vec::new();
        if let Ok(metadata) = self.client.metadata().entity(target_entity.as_str()).await {
            for attr in &metadata.attributes {
                let type_str = format!("{:?}", attr.attribute_type);
                let label = format!("{} ({})", attr.logical_name, type_str);
                suggestions.push((attr.logical_name.clone(), label));
            }
        }

        suggestions
    }

    /// Generate polymorphic target suggestions when inside brackets.
    async fn generate_polymorphic_targets(&self, prefix: &str) -> Vec<(String, String)> {
        // Parse to find the lookup field
        let segments: Vec<&str> = prefix.split('.').collect();
        let lookup_field = segments.last().map(|s| s.trim()).unwrap_or("");

        if lookup_field.is_empty() {
            return Vec::new();
        }

        // Determine which entity to query
        let entity_name = if segments.len() == 1 {
            // Root level lookup
            self.source_entity.clone()
        } else {
            // Need to resolve the entity up to this point
            let path_before = segments[..segments.len() - 1].join(".");
            match self.resolve_entity_at_path(&path_before).await {
                Some(e) => e,
                None => return Vec::new(),
            }
        };

        // Fetch metadata and get targets
        if let Ok(metadata) = self.client.metadata().entity(entity_name.as_str()).await {
            if let Some(attr) = metadata.attribute(lookup_field) {
                return attr
                    .targets
                    .iter()
                    .map(|target| {
                        let label = format!("{} (target)", target);
                        (target.clone(), label)
                    })
                    .collect();
            }
        }

        Vec::new()
    }

    /// Resolve what entity we're on at a given path position.
    async fn resolve_entity_at_path(&self, path: &str) -> Option<String> {
        if path.is_empty() {
            return Some(self.source_entity.clone());
        }

        // Parse the path
        let parsed = match parse_path(path) {
            Ok(PathExpr::Field(field_path)) => field_path,
            _ => return None,
        };

        let mut current_entity = self.source_entity.clone();

        // Walk through segments (excluding the last one if it's a field)
        for segment in &parsed.segments {
            let metadata = self.client.metadata().entity(current_entity.as_str()).await.ok()?;
            let attr = metadata.attribute(&segment.field)?;

            // If this is a lookup, follow it
            if is_lookup_type(attr.attribute_type) {
                if attr.targets.len() > 1 {
                    // Polymorphic - need target specifier
                    current_entity = segment.target.clone()?;
                } else {
                    current_entity = attr.targets.first()?.clone();
                }
            } else {
                // Not a lookup, can't navigate further
                return None;
            }
        }

        Some(current_entity)
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
            variables: self.variables.clone(),
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

/// Check if an attribute type is a lookup type.
fn is_lookup_type(attr_type: AttributeType) -> bool {
    matches!(
        attr_type,
        AttributeType::Lookup | AttributeType::Customer | AttributeType::Owner
    )
}
