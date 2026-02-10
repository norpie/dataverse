//! Shared path suggestion logic for Copy and Format transform modals.

use dataverse_lib::DataverseClient;
use dataverse_lib::model::FieldType;
use dataverse_lib::model::ValueType;
use dataverse_lib::model::metadata::AttributeType;
use log::debug;

use crate::apps::migration::validation::PathExpr;
use crate::apps::migration::validation::parse_path;

/// A variable with its name and declared type, for path suggestions.
#[derive(Clone, Debug)]
pub struct VariableInfo {
    pub name: String,
    pub declared_type: ValueType,
}

/// Generator for path autocomplete suggestions.
///
/// Used by both Copy and Format transform modals to suggest field paths,
/// variables, and system variables.
pub struct PathSuggestionGenerator {
    client: DataverseClient,
    source_entity: String,
    variables: Vec<VariableInfo>,
}

impl PathSuggestionGenerator {
    /// Create a new path suggestion generator.
    pub fn new(
        client: DataverseClient,
        source_entity: String,
        variables: Vec<VariableInfo>,
    ) -> Self {
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
            let value = format!("${}", var.name);
            let type_hint = format!("{}", var.declared_type);
            let label = format!("${} ({})", var.name, type_hint);
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

        // Check if prefix is a variable: "$varname" or "$varname.field...field"
        if let Some(var_rest) = prefix.strip_prefix('$') {
            // The last segment is the lookup — either the variable itself or a field after dots
            if let Some(dot_pos) = var_rest.rfind('.') {
                // "$var.some.field[" — resolve entity at "$var.some", then get field's targets
                let path_before_last = format!("${}", &var_rest[..dot_pos]);
                let lookup_field = &var_rest[dot_pos + 1..];
                debug!(
                    "[PathSuggestions] Variable nested polymorphic: path_before={:?}, lookup_field={:?}",
                    path_before_last, lookup_field
                );
                let entity = match self.resolve_entity_at_path(&path_before_last).await {
                    Some(e) => e,
                    None => return Vec::new(),
                };
                return self.fetch_field_targets(&entity, lookup_field).await;
            } else {
                // "$var[" — the variable itself is the lookup, return its targets
                let var_name = var_rest;
                debug!(
                    "[PathSuggestions] Variable root polymorphic: var_name={:?}",
                    var_name
                );
                if let Some(var) = self.variables.iter().find(|v| v.name == var_name) {
                    if let ValueType::Known(FieldType::Lookup { targets, .. }) = &var.declared_type
                    {
                        return targets
                            .iter()
                            .map(|t| {
                                let label = format!("{} (target)", t);
                                (t.clone(), label)
                            })
                            .collect();
                    }
                }
                return Vec::new();
            }
        }

        // Regular field path: parse to find the lookup field
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

        self.fetch_field_targets(&entity_name, lookup_field).await
    }

    /// Fetch the target entities for a lookup field on an entity.
    async fn fetch_field_targets(&self, entity: &str, field: &str) -> Vec<(String, String)> {
        match self.client.metadata().entity(entity).await {
            Ok(metadata) => {
                debug!("[PathSuggestions] Fetched metadata for {}", entity);
                if let Some(attr) = metadata.attribute(field) {
                    debug!(
                        "[PathSuggestions] Found attribute {:?} with targets: {:?}",
                        field, attr.targets
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
                        field, entity
                    );
                }
            }
            Err(e) => {
                debug!(
                    "[PathSuggestions] Failed to fetch metadata for {}: {:?}",
                    entity, e
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
            Ok(parsed) => parsed,
            Err(e) => {
                debug!("[PathSuggestions] Failed to parse path: {:?}", e);
                // Fallback for partial variable paths like "$var[target]" that the strict
                // parser rejects (it requires ".field" after "]"). For suggestion purposes
                // we just need to resolve to the target entity.
                if let Some(rest) = path.strip_prefix('$') {
                    return self.resolve_partial_variable_path(rest);
                }
                return None;
            }
        };

        // Determine the starting entity and field path based on parsed expression
        let (mut current_entity, field_path) = match parsed {
            PathExpr::Field(field_path) => {
                debug!(
                    "[PathSuggestions] Parsed field path with {} segments",
                    field_path.segments.len()
                );
                (self.source_entity.clone(), field_path)
            }
            PathExpr::Variable(name) => {
                // A bare variable — resolve to its Lookup target entity
                debug!(
                    "[PathSuggestions] Parsed variable '{}', resolving to target entity",
                    name
                );
                return self.resolve_variable_target_entity(&name, None);
            }
            PathExpr::VariableNavigation { name, target, path } => {
                // Variable with field navigation — start from variable's target entity
                let start_entity = self.resolve_variable_target_entity(&name, target.as_deref())?;
                debug!(
                    "[PathSuggestions] Parsed variable navigation '{}' -> entity '{}', {} path segments",
                    name,
                    start_entity,
                    path.segments.len()
                );
                (start_entity, path)
            }
            PathExpr::SystemVar(_) => {
                debug!("[PathSuggestions] System variables don't resolve to entities");
                return None;
            }
        };

        // Walk through segments (excluding the last one if it's a field)
        for segment in &field_path.segments {
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

    /// Fallback for partial variable paths that the strict parser rejects.
    ///
    /// Handles paths like `var[target]` (after `$` is stripped) that `parse_path`
    /// rejects because there's no `.field` after `]`.
    fn resolve_partial_variable_path(&self, rest: &str) -> Option<String> {
        // Check for bracket target: "var[target]"
        if let Some(bracket_start) = rest.find('[') {
            let name = &rest[..bracket_start];
            let after = &rest[bracket_start + 1..];
            if let Some(bracket_end) = after.find(']') {
                let target = &after[..bracket_end];
                if !name.is_empty() && !target.is_empty() {
                    debug!(
                        "[PathSuggestions] Partial variable path: name={:?}, target={:?}",
                        name, target
                    );
                    return self.resolve_variable_target_entity(name, Some(target));
                }
            }
        }
        // Plain variable name without brackets
        if !rest.is_empty() && !rest.contains('.') {
            debug!(
                "[PathSuggestions] Partial variable path (plain): name={:?}",
                rest
            );
            return self.resolve_variable_target_entity(rest, None);
        }
        None
    }

    /// Resolve a variable to its target entity name.
    ///
    /// For a variable typed as `Lookup(account)`, returns `"account"`.
    /// For polymorphic lookups `Lookup(account, contact)`, `target` disambiguates.
    fn resolve_variable_target_entity(&self, name: &str, target: Option<&str>) -> Option<String> {
        let var = self.variables.iter().find(|v| v.name == name)?;
        let targets = match &var.declared_type {
            ValueType::Known(FieldType::Lookup { targets, .. }) => targets,
            _ => {
                debug!(
                    "[PathSuggestions] Variable '{}' type {:?} is not a Lookup",
                    name, var.declared_type
                );
                return None;
            }
        };

        if targets.is_empty() {
            debug!(
                "[PathSuggestions] Variable '{}' Lookup has no targets",
                name
            );
            return None;
        }

        if let Some(specified) = target {
            // Polymorphic disambiguation
            if targets.contains(&specified.to_string()) {
                Some(specified.to_string())
            } else {
                debug!(
                    "[PathSuggestions] Variable '{}' target '{}' not in {:?}",
                    name, specified, targets
                );
                None
            }
        } else if targets.len() == 1 {
            Some(targets[0].clone())
        } else {
            debug!(
                "[PathSuggestions] Variable '{}' is polymorphic ({:?}) but no target specified",
                name, targets
            );
            None
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
