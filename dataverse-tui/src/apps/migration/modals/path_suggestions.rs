//! Shared path suggestion logic for Copy and Format transform modals.

use dataverse_lib::model::metadata::AttributeType;
use dataverse_lib::DataverseClient;
use log::debug;

use crate::apps::migration::validation::parse_path;
use crate::apps::migration::validation::PathExpr;

/// Generator for path autocomplete suggestions.
///
/// Used by both Copy and Format transform modals to suggest field paths,
/// variables, and system variables.
pub struct PathSuggestionGenerator {
    client: DataverseClient,
    source_entity: String,
    variables: Vec<String>,
}

impl PathSuggestionGenerator {
    /// Create a new path suggestion generator.
    pub fn new(client: DataverseClient, source_entity: String, variables: Vec<String>) -> Self {
        Self {
            client,
            source_entity,
            variables,
        }
    }

    /// Generate autocomplete suggestions based on current input.
    ///
    /// Returns (value, label) pairs where value is the COMPLETE path.
    /// The autocomplete widget replaces the entire input with the selected value.
    pub async fn generate_suggestions(&self, input: &str) -> Vec<(String, String)> {
        let input = input.trim();
        debug!(
            "[PathSuggestions] generate_suggestions called with input: {:?}",
            input
        );

        // Case 1: Inside unclosed brackets - suggest polymorphic targets
        if let Some(bracket_pos) = input.rfind('[') {
            if !input[bracket_pos..].contains(']') {
                let path_before_bracket = &input[..bracket_pos];
                let prefix = &input[..=bracket_pos]; // e.g., "ownerid["
                debug!(
                    "[PathSuggestions] Case 1: Inside brackets. path_before_bracket={:?}, prefix={:?}",
                    path_before_bracket, prefix
                );
                let targets = self.generate_polymorphic_targets(path_before_bracket).await;
                debug!(
                    "[PathSuggestions] Polymorphic targets returned: {:?}",
                    targets
                );
                let result: Vec<_> = targets
                    .into_iter()
                    .map(|(target, _type_label)| {
                        // Full path with closing bracket: "ownerid[systemuser]"
                        let full_path = format!("{}{}]", prefix, target);
                        // Label is also the full path (so fuzzy filter works)
                        (full_path.clone(), full_path)
                    })
                    .collect();
                debug!(
                    "[PathSuggestions] Final suggestions for brackets: {:?}",
                    result
                );
                return result;
            }
        }

        // Case 2: Has a dot - suggest fields of the resolved entity
        if let Some(dot_pos) = input.rfind('.') {
            let path_before_dot = &input[..dot_pos];
            let prefix = &input[..=dot_pos]; // e.g., "parentaccountid."
            debug!(
                "[PathSuggestions] Case 2: After dot. path_before_dot={:?}, prefix={:?}",
                path_before_dot, prefix
            );
            let fields = self.generate_field_suggestions(path_before_dot).await;
            debug!(
                "[PathSuggestions] Field suggestions returned: {} items",
                fields.len()
            );
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
            debug!(
                "[PathSuggestions] Final suggestions for dot: {} items",
                result.len()
            );
            return result;
        }

        // Case 3: Root level - no dots, no unclosed brackets
        debug!("[PathSuggestions] Case 3: Root level");
        let result = self.generate_root_suggestions().await;
        debug!(
            "[PathSuggestions] Root suggestions returned: {} items",
            result.len()
        );
        result
    }

    /// Generate root-level suggestions: source fields, variables, system vars.
    pub async fn generate_root_suggestions(&self) -> Vec<(String, String)> {
        let mut suggestions = Vec::new();

        // Fetch source entity metadata
        if let Ok(metadata) = self
            .client
            .metadata()
            .entity(self.source_entity.as_str())
            .await
        {
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
    pub async fn generate_field_suggestions(&self, path: &str) -> Vec<(String, String)> {
        debug!(
            "[PathSuggestions] generate_field_suggestions called with path: {:?}",
            path
        );

        // Try to determine what entity we're on after following this path
        let target_entity = match self.resolve_entity_at_path(path).await {
            Some(entity) => {
                debug!("[PathSuggestions] Resolved path to entity: {:?}", entity);
                entity
            }
            None => {
                debug!("[PathSuggestions] Failed to resolve path to entity");
                return Vec::new();
            }
        };

        // Fetch fields of target entity
        let mut suggestions = Vec::new();
        match self.client.metadata().entity(target_entity.as_str()).await {
            Ok(metadata) => {
                debug!(
                    "[PathSuggestions] Fetched metadata for {}, {} attributes",
                    target_entity,
                    metadata.attributes.len()
                );
                for attr in &metadata.attributes {
                    let type_str = format!("{:?}", attr.attribute_type);
                    // Return (field_name, type_string) - caller builds full label
                    suggestions.push((attr.logical_name.clone(), type_str));
                }
            }
            Err(e) => {
                debug!(
                    "[PathSuggestions] Failed to fetch metadata for {}: {:?}",
                    target_entity, e
                );
            }
        }

        suggestions
    }

    /// Generate polymorphic target suggestions when inside brackets.
    pub async fn generate_polymorphic_targets(&self, prefix: &str) -> Vec<(String, String)> {
        debug!(
            "[PathSuggestions] generate_polymorphic_targets called with prefix: {:?}",
            prefix
        );

        // Parse to find the lookup field
        let segments: Vec<&str> = prefix.split('.').collect();
        let lookup_field = segments.last().map(|s| s.trim()).unwrap_or("");
        debug!(
            "[PathSuggestions] segments: {:?}, lookup_field: {:?}",
            segments, lookup_field
        );

        if lookup_field.is_empty() {
            debug!("[PathSuggestions] lookup_field is empty, returning empty");
            return Vec::new();
        }

        // Determine which entity to query
        let entity_name = if segments.len() == 1 {
            // Root level lookup
            debug!(
                "[PathSuggestions] Root level lookup, using source_entity: {:?}",
                self.source_entity
            );
            self.source_entity.clone()
        } else {
            // Need to resolve the entity up to this point
            let path_before = segments[..segments.len() - 1].join(".");
            debug!(
                "[PathSuggestions] Nested lookup, resolving path_before: {:?}",
                path_before
            );
            match self.resolve_entity_at_path(&path_before).await {
                Some(e) => {
                    debug!("[PathSuggestions] Resolved to entity: {:?}", e);
                    e
                }
                None => {
                    debug!("[PathSuggestions] Failed to resolve path_before");
                    return Vec::new();
                }
            }
        };

        // Fetch metadata and get targets
        match self.client.metadata().entity(entity_name.as_str()).await {
            Ok(metadata) => {
                debug!("[PathSuggestions] Fetched metadata for {}", entity_name);
                if let Some(attr) = metadata.attribute(lookup_field) {
                    debug!(
                        "[PathSuggestions] Found attribute {:?} with targets: {:?}",
                        lookup_field, attr.targets
                    );
                    return attr
                        .targets
                        .iter()
                        .map(|target| {
                            let label = format!("{} (target)", target);
                            (target.clone(), label)
                        })
                        .collect();
                } else {
                    debug!(
                        "[PathSuggestions] Attribute {:?} not found on {}",
                        lookup_field, entity_name
                    );
                }
            }
            Err(e) => {
                debug!(
                    "[PathSuggestions] Failed to fetch metadata for {}: {:?}",
                    entity_name, e
                );
            }
        }

        Vec::new()
    }

    /// Resolve what entity we're on at a given path position.
    pub async fn resolve_entity_at_path(&self, path: &str) -> Option<String> {
        debug!(
            "[PathSuggestions] resolve_entity_at_path called with path: {:?}",
            path
        );

        if path.is_empty() {
            debug!(
                "[PathSuggestions] Path is empty, returning source_entity: {:?}",
                self.source_entity
            );
            return Some(self.source_entity.clone());
        }

        // Parse the path
        let parsed = match parse_path(path) {
            Ok(PathExpr::Field(field_path)) => {
                debug!(
                    "[PathSuggestions] Parsed path into {} segments",
                    field_path.segments.len()
                );
                field_path
            }
            Ok(other) => {
                debug!(
                    "[PathSuggestions] Path parsed but not a field path: {:?}",
                    other
                );
                return None;
            }
            Err(e) => {
                debug!("[PathSuggestions] Failed to parse path: {:?}", e);
                return None;
            }
        };

        let mut current_entity = self.source_entity.clone();

        // Walk through segments (excluding the last one if it's a field)
        for segment in &parsed.segments {
            debug!(
                "[PathSuggestions] Processing segment: field={:?}, target={:?}, optional={:?}",
                segment.field, segment.target, segment.optional
            );

            let metadata = match self.client.metadata().entity(current_entity.as_str()).await {
                Ok(m) => m,
                Err(e) => {
                    debug!(
                        "[PathSuggestions] Failed to get metadata for {}: {:?}",
                        current_entity, e
                    );
                    return None;
                }
            };

            let attr = match metadata.attribute(&segment.field) {
                Some(a) => a,
                None => {
                    debug!(
                        "[PathSuggestions] Attribute {:?} not found on {}",
                        segment.field, current_entity
                    );
                    return None;
                }
            };

            // If this is a lookup, follow it
            if is_lookup_type(attr.attribute_type) {
                if attr.targets.len() > 1 {
                    // Polymorphic - need target specifier
                    match &segment.target {
                        Some(t) => {
                            debug!(
                                "[PathSuggestions] Polymorphic lookup, using target: {:?}",
                                t
                            );
                            current_entity = t.clone();
                        }
                        None => {
                            debug!("[PathSuggestions] Polymorphic lookup but no target specified");
                            return None;
                        }
                    }
                } else {
                    match attr.targets.first() {
                        Some(t) => {
                            debug!(
                                "[PathSuggestions] Single-target lookup, following to: {:?}",
                                t
                            );
                            current_entity = t.clone();
                        }
                        None => {
                            debug!("[PathSuggestions] Lookup has no targets");
                            return None;
                        }
                    }
                }
            } else {
                // Not a lookup, can't navigate further
                debug!(
                    "[PathSuggestions] Attribute {:?} is not a lookup type: {:?}",
                    segment.field, attr.attribute_type
                );
                return None;
            }
        }

        debug!("[PathSuggestions] Resolved to entity: {:?}", current_entity);
        Some(current_entity)
    }
}

/// Check if an attribute type is a lookup type.
fn is_lookup_type(attr_type: AttributeType) -> bool {
    matches!(
        attr_type,
        AttributeType::Lookup | AttributeType::Customer | AttributeType::Owner
    )
}
