//! Modal for editing a Copy transform.

use dataverse_lib::model::metadata::AttributeType;
use dataverse_lib::DataverseClient;
use rafter::page;
use rafter::prelude::*;
use rafter::widgets::Autocomplete;
use rafter::widgets::AutocompleteState;
use rafter::widgets::Button;
use rafter::widgets::Text;
use log::debug;
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
        debug!("[CopyTransform] on_path_change triggered, path: {:?}", path);
        
        // Generate new suggestions based on current input
        let suggestions = self.generate_suggestions(&path).await;
        debug!("[CopyTransform] Generated {} suggestions, updating autocomplete", suggestions.len());
        
        self.autocomplete.update(|s| {
            s.options = suggestions;
            s.refilter();
        });
        
        // Validate
        self.validate_path().await;
    }

    /// Generate autocomplete suggestions based on current input.
    ///
    /// Returns (value, label) pairs where value is the COMPLETE path (not just the segment).
    /// The autocomplete widget replaces the entire input with the selected value.
    async fn generate_suggestions(&self, input: &str) -> Vec<(String, String)> {
        let input = input.trim();
        debug!("[CopyTransform] generate_suggestions called with input: {:?}", input);

        // Case 1: Inside unclosed brackets - suggest polymorphic targets
        if let Some(bracket_pos) = input.rfind('[') {
            if !input[bracket_pos..].contains(']') {
                let path_before_bracket = &input[..bracket_pos];
                let prefix = &input[..=bracket_pos]; // e.g., "ownerid["
                debug!(
                    "[CopyTransform] Case 1: Inside brackets. path_before_bracket={:?}, prefix={:?}",
                    path_before_bracket, prefix
                );
                let targets = self.generate_polymorphic_targets(path_before_bracket).await;
                debug!("[CopyTransform] Polymorphic targets returned: {:?}", targets);
                let result: Vec<_> = targets
                    .into_iter()
                    .map(|(target, _type_label)| {
                        // Full path with closing bracket: "ownerid[systemuser]"
                        let full_path = format!("{}{}]", prefix, target);
                        // Label is also the full path (so fuzzy filter works)
                        (full_path.clone(), full_path)
                    })
                    .collect();
                debug!("[CopyTransform] Final suggestions for brackets: {:?}", result);
                return result;
            }
        }

        // Case 2: Has a dot - suggest fields of the resolved entity
        if let Some(dot_pos) = input.rfind('.') {
            let path_before_dot = &input[..dot_pos];
            let prefix = &input[..=dot_pos]; // e.g., "parentaccountid."
            debug!(
                "[CopyTransform] Case 2: After dot. path_before_dot={:?}, prefix={:?}",
                path_before_dot, prefix
            );
            let fields = self.generate_field_suggestions(path_before_dot).await;
            debug!("[CopyTransform] Field suggestions returned: {} items", fields.len());
            let result: Vec<_> = fields
                .into_iter()
                .map(|(field, type_info)| {
                    // Full path: "parentaccountid.name"
                    let full_path = format!("{}{}", prefix, field);
                    // Label shows path + type info for display
                    let label = format!("{} ({})", full_path, type_info);
                    (full_path, label)
                })
                .collect();
            debug!("[CopyTransform] Final suggestions for dot: {} items", result.len());
            return result;
        }

        // Case 3: Root level - no dots, no unclosed brackets
        debug!("[CopyTransform] Case 3: Root level");
        let result = self.generate_root_suggestions().await;
        debug!("[CopyTransform] Root suggestions returned: {} items", result.len());
        result
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

    /// Generate field suggestions for fields of the entity at the given path.
    ///
    /// `path` is the complete path to a lookup field (e.g., "parentaccountid" or "ownerid[systemuser]").
    /// Returns (field_name, type_string) pairs - caller builds the full label.
    async fn generate_field_suggestions(&self, path: &str) -> Vec<(String, String)> {
        debug!("[CopyTransform] generate_field_suggestions called with path: {:?}", path);
        
        // Try to determine what entity we're on after following this path
        let target_entity = match self.resolve_entity_at_path(path).await {
            Some(entity) => {
                debug!("[CopyTransform] Resolved path to entity: {:?}", entity);
                entity
            }
            None => {
                debug!("[CopyTransform] Failed to resolve path to entity");
                return Vec::new();
            }
        };

        // Fetch fields of target entity
        let mut suggestions = Vec::new();
        match self.client.metadata().entity(target_entity.as_str()).await {
            Ok(metadata) => {
                debug!("[CopyTransform] Fetched metadata for {}, {} attributes", target_entity, metadata.attributes.len());
                for attr in &metadata.attributes {
                    let type_str = format!("{:?}", attr.attribute_type);
                    // Return (field_name, type_string) - caller builds full label
                    suggestions.push((attr.logical_name.clone(), type_str));
                }
            }
            Err(e) => {
                debug!("[CopyTransform] Failed to fetch metadata for {}: {:?}", target_entity, e);
            }
        }

        suggestions
    }

    /// Generate polymorphic target suggestions when inside brackets.
    async fn generate_polymorphic_targets(&self, prefix: &str) -> Vec<(String, String)> {
        debug!("[CopyTransform] generate_polymorphic_targets called with prefix: {:?}", prefix);
        
        // Parse to find the lookup field
        let segments: Vec<&str> = prefix.split('.').collect();
        let lookup_field = segments.last().map(|s| s.trim()).unwrap_or("");
        debug!("[CopyTransform] segments: {:?}, lookup_field: {:?}", segments, lookup_field);

        if lookup_field.is_empty() {
            debug!("[CopyTransform] lookup_field is empty, returning empty");
            return Vec::new();
        }

        // Determine which entity to query
        let entity_name = if segments.len() == 1 {
            // Root level lookup
            debug!("[CopyTransform] Root level lookup, using source_entity: {:?}", self.source_entity);
            self.source_entity.clone()
        } else {
            // Need to resolve the entity up to this point
            let path_before = segments[..segments.len() - 1].join(".");
            debug!("[CopyTransform] Nested lookup, resolving path_before: {:?}", path_before);
            match self.resolve_entity_at_path(&path_before).await {
                Some(e) => {
                    debug!("[CopyTransform] Resolved to entity: {:?}", e);
                    e
                }
                None => {
                    debug!("[CopyTransform] Failed to resolve path_before");
                    return Vec::new();
                }
            }
        };

        // Fetch metadata and get targets
        match self.client.metadata().entity(entity_name.as_str()).await {
            Ok(metadata) => {
                debug!("[CopyTransform] Fetched metadata for {}", entity_name);
                if let Some(attr) = metadata.attribute(lookup_field) {
                    debug!("[CopyTransform] Found attribute {:?} with targets: {:?}", lookup_field, attr.targets);
                    return attr
                        .targets
                        .iter()
                        .map(|target| {
                            let label = format!("{} (target)", target);
                            (target.clone(), label)
                        })
                        .collect();
                } else {
                    debug!("[CopyTransform] Attribute {:?} not found on {}", lookup_field, entity_name);
                }
            }
            Err(e) => {
                debug!("[CopyTransform] Failed to fetch metadata for {}: {:?}", entity_name, e);
            }
        }

        Vec::new()
    }

    /// Resolve what entity we're on at a given path position.
    async fn resolve_entity_at_path(&self, path: &str) -> Option<String> {
        debug!("[CopyTransform] resolve_entity_at_path called with path: {:?}", path);
        
        if path.is_empty() {
            debug!("[CopyTransform] Path is empty, returning source_entity: {:?}", self.source_entity);
            return Some(self.source_entity.clone());
        }

        // Parse the path
        let parsed = match parse_path(path) {
            Ok(PathExpr::Field(field_path)) => {
                debug!("[CopyTransform] Parsed path into {} segments", field_path.segments.len());
                field_path
            }
            Ok(other) => {
                debug!("[CopyTransform] Path parsed but not a field path: {:?}", other);
                return None;
            }
            Err(e) => {
                debug!("[CopyTransform] Failed to parse path: {:?}", e);
                return None;
            }
        };

        let mut current_entity = self.source_entity.clone();

        // Walk through segments (excluding the last one if it's a field)
        for segment in &parsed.segments {
            debug!(
                "[CopyTransform] Processing segment: field={:?}, target={:?}, optional={:?}",
                segment.field, segment.target, segment.optional
            );
            
            let metadata = match self.client.metadata().entity(current_entity.as_str()).await {
                Ok(m) => m,
                Err(e) => {
                    debug!("[CopyTransform] Failed to get metadata for {}: {:?}", current_entity, e);
                    return None;
                }
            };
            
            let attr = match metadata.attribute(&segment.field) {
                Some(a) => a,
                None => {
                    debug!("[CopyTransform] Attribute {:?} not found on {}", segment.field, current_entity);
                    return None;
                }
            };

            // If this is a lookup, follow it
            if is_lookup_type(attr.attribute_type) {
                if attr.targets.len() > 1 {
                    // Polymorphic - need target specifier
                    match &segment.target {
                        Some(t) => {
                            debug!("[CopyTransform] Polymorphic lookup, using target: {:?}", t);
                            current_entity = t.clone();
                        }
                        None => {
                            debug!("[CopyTransform] Polymorphic lookup but no target specified");
                            return None;
                        }
                    }
                } else {
                    match attr.targets.first() {
                        Some(t) => {
                            debug!("[CopyTransform] Single-target lookup, following to: {:?}", t);
                            current_entity = t.clone();
                        }
                        None => {
                            debug!("[CopyTransform] Lookup has no targets");
                            return None;
                        }
                    }
                }
            } else {
                // Not a lookup, can't navigate further
                debug!("[CopyTransform] Attribute {:?} is not a lookup type: {:?}", segment.field, attr.attribute_type);
                return None;
            }
        }

        debug!("[CopyTransform] Resolved to entity: {:?}", current_entity);
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
