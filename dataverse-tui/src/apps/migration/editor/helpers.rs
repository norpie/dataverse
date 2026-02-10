//! Internal helper functions for the migration editor.

use dataverse_lib::DataverseClient;
use dataverse_lib::model::FieldType;
use dataverse_lib::model::ValueType;
use dataverse_lib::model::metadata::AttributeType;
use rafter::prelude::*;

use super::MigrationEditor;
use super::tree::FieldTypeCache;
use super::tree::MigrationTreeNode;
use crate::apps::migration::repository::MigrationRepository;
use crate::apps::migration::types::TransformData;
use crate::apps::migration::validation::FieldPath;
use crate::apps::migration::validation::PathExpr;
use crate::apps::migration::validation::parse_path;

impl MigrationEditor {
    /// Load all data from the repository.
    /// Uses bulk queries to avoid N+1 (8 queries total).
    /// The `#[watch] rebuild` method auto-triggers after state changes.
    pub(super) async fn load_db_data(&self, gx: &GlobalContext) {
        let migration_id = self.migration.get().id;
        let repo = gx.data::<MigrationRepository>();

        // Reload all data with bulk queries
        if let Ok(phases) = repo.get_phases(migration_id).await {
            self.phases.set(phases);
        }

        if let Ok(mappings) = repo.get_entity_mappings_by_migration(migration_id).await {
            self.entity_mappings.set(mappings);
        }

        if let Ok(vars) = repo.get_variables_by_migration(migration_id).await {
            self.variables.set(vars);
        }

        if let Ok(fms) = repo.get_field_mappings_by_migration(migration_id).await {
            self.field_mappings.set(fms);
        }

        if let Ok(transforms) = repo.get_transforms_by_migration(migration_id).await {
            self.transforms.set(transforms);
        }

        if let Ok(branches) = repo.get_match_branches_by_migration(migration_id).await {
            self.match_branches.set(branches);
        }

        if let Ok(chains) = repo.get_coalesce_chains_by_migration(migration_id).await {
            self.coalesce_chains.set(chains);
        }

        if let Ok(conditions) = repo.get_find_conditions_by_migration(migration_id).await {
            self.find_conditions.set(conditions);
        }

        if let Ok(conditions) = repo.get_match_conditions_by_migration(migration_id).await {
            self.match_conditions.set(conditions);
        }
    }

    /// Get the currently focused tree node.
    pub(super) fn focused_node(&self) -> Option<MigrationTreeNode> {
        self.tree_state.with_ref(|s| {
            s.focused_key
                .as_ref()
                .and_then(|key| s.find_node(key))
                .map(|node| node.value.clone())
        })
    }

    /// Count entity mappings for a phase.
    pub(super) fn entity_count_for_phase(&self, phase_id: i64) -> usize {
        self.entity_mappings
            .get()
            .iter()
            .filter(|em| em.phase_id == phase_id)
            .count()
    }
}

/// Fetch attribute types for multiple entities.
pub(super) async fn fetch_entity_field_types(
    client: DataverseClient,
    entities: Vec<String>,
) -> FieldTypeCache {
    let mut cache = FieldTypeCache::new();

    for entity_name in entities {
        match client.metadata().entity(entity_name.as_str()).await {
            Ok(metadata) => {
                let fields: std::collections::HashMap<String, FieldType> = metadata
                    .attributes
                    .iter()
                    .map(|attr| {
                        let field_type = match attr.attribute_type {
                            // For option set types, use the typed attribute accessors
                            // which have the full option set metadata (name + options).
                            AttributeType::Picklist => {
                                resolve_picklist_field_type(&metadata, &attr.logical_name)
                            }
                            AttributeType::State => {
                                resolve_state_field_type(&metadata, &attr.logical_name)
                            }
                            AttributeType::Status => {
                                resolve_status_field_type(&metadata, &attr.logical_name)
                            }
                            AttributeType::MultiSelectPicklist => {
                                resolve_multi_select_field_type(&metadata, &attr.logical_name)
                            }
                            // All other types use the base AttributeMetadata conversion.
                            _ => FieldType::from(attr),
                        };
                        (attr.logical_name.clone(), field_type)
                    })
                    .collect();
                log::debug!(
                    "type_tracking: fetched {} fields for entity '{}'",
                    fields.len(),
                    entity_name,
                );
                cache.insert(entity_name, fields);
            }
            Err(e) => {
                log::warn!(
                    "type_tracking: failed to fetch metadata for '{}': {}",
                    entity_name,
                    e,
                );
            }
        }
    }

    cache
}

/// Resolve a picklist field to a `FieldType::OptionSet` using the typed attribute accessor.
fn resolve_picklist_field_type(
    metadata: &dataverse_lib::model::metadata::EntityMetadata,
    logical_name: &str,
) -> FieldType {
    let name = metadata
        .picklist_attribute(logical_name)
        .and_then(|typed| typed.option_set.name.clone())
        .unwrap_or_default();
    FieldType::OptionSet {
        kind: AttributeType::Picklist,
        name,
    }
}

/// Resolve a state field to a `FieldType::OptionSet` using the typed attribute accessor.
fn resolve_state_field_type(
    metadata: &dataverse_lib::model::metadata::EntityMetadata,
    logical_name: &str,
) -> FieldType {
    let name = metadata
        .state_attribute(logical_name)
        .and_then(|typed| typed.option_set.name.clone())
        .unwrap_or_default();
    FieldType::OptionSet {
        kind: AttributeType::State,
        name,
    }
}

/// Resolve a status field to a `FieldType::OptionSet` using the typed attribute accessor.
fn resolve_status_field_type(
    metadata: &dataverse_lib::model::metadata::EntityMetadata,
    logical_name: &str,
) -> FieldType {
    let name = metadata
        .status_attribute(logical_name)
        .and_then(|typed| typed.option_set.name.clone())
        .unwrap_or_default();
    FieldType::OptionSet {
        kind: AttributeType::Status,
        name,
    }
}

/// Resolve a multi-select picklist field to a `FieldType::OptionSet` using the typed attribute accessor.
fn resolve_multi_select_field_type(
    metadata: &dataverse_lib::model::metadata::EntityMetadata,
    logical_name: &str,
) -> FieldType {
    let name = metadata
        .multi_select_picklist_attribute(logical_name)
        .and_then(|typed| typed.option_set.name.clone())
        .unwrap_or_default();
    FieldType::OptionSet {
        kind: AttributeType::MultiSelectPicklist,
        name,
    }
}

/// A navigation path that requires entity metadata fetching.
pub(super) struct NavigationPath {
    /// The entity to start navigating from.
    pub start_entity: String,
    /// The field path segments to navigate through.
    pub path: FieldPath,
}

/// Collect all paths that require navigation entity metadata.
///
/// Includes:
/// - Dotted field paths (`parentaccountid.name`) — start from the source entity
/// - Variable navigation paths (`$var.field`) — start from the variable's Lookup target entity
pub(super) fn collect_navigation_paths(
    transforms: &[crate::apps::migration::types::Transform],
    entity_mappings: &[crate::apps::migration::types::EntityMapping],
    variables: &[crate::apps::migration::types::Variable],
) -> Vec<NavigationPath> {
    let mut paths = Vec::new();

    for t in transforms {
        if let TransformData::Copy { path } = &t.data {
            if path.starts_with('#') {
                continue;
            }

            match parse_path(path) {
                Ok(PathExpr::Field(field_path)) if field_path.segments.len() >= 2 => {
                    // Dotted field path — start from source entity
                    let source_entity = entity_mappings
                        .iter()
                        .find(|em| em.id == t.entity_mapping_id)
                        .map(|em| em.source_entity.clone())
                        .unwrap_or_default();
                    if !source_entity.is_empty() {
                        paths.push(NavigationPath {
                            start_entity: source_entity,
                            path: field_path,
                        });
                    }
                }
                Ok(PathExpr::VariableNavigation {
                    name,
                    target,
                    path: field_path,
                }) => {
                    // Variable navigation — resolve target entity from variable's declared type
                    let var = variables
                        .iter()
                        .filter(|v| v.entity_mapping_id == t.entity_mapping_id)
                        .find(|v| v.name == name);

                    if let Some(var) = var
                        && let Some(start_entity) =
                            resolve_lookup_target_entity(&var.declared_type, target.as_deref())
                        {
                            paths.push(NavigationPath {
                                start_entity,
                                path: field_path,
                            });
                        }
                }
                _ => {}
            }
        }
    }

    paths
}

/// Resolve the target entity from a Lookup ValueType, with optional polymorphic target.
fn resolve_lookup_target_entity(value_type: &ValueType, target: Option<&str>) -> Option<String> {
    let targets = match value_type {
        ValueType::Known(FieldType::Lookup { targets, .. }) => targets,
        ValueType::Union(types) => {
            
            types.iter().find_map(|ft| match ft {
                FieldType::Lookup { targets, .. } => Some(targets),
                _ => None,
            })?
        }
        _ => return None,
    };

    if let Some(specified) = target {
        if targets.contains(&specified.to_string()) {
            Some(specified.to_string())
        } else {
            None
        }
    } else if targets.len() == 1 {
        Some(targets[0].clone())
    } else {
        None
    }
}

/// Discover navigation entities that are not yet in the source cache.
///
/// Walks each navigation path segment-by-segment using the cached metadata. For each
/// lookup segment, collects the target entity names. Returns entities that are
/// not yet in the cache.
pub(super) fn discover_navigation_entities(
    nav_paths: &[NavigationPath],
    source_cache: &FieldTypeCache,
) -> Vec<String> {
    let mut missing: Vec<String> = Vec::new();

    for np in nav_paths {
        if np.start_entity.is_empty() {
            continue;
        }

        // Ensure the start entity itself is cached
        if !source_cache.contains_key(&np.start_entity) && !missing.contains(&np.start_entity) {
            missing.push(np.start_entity.clone());
            continue;
        }

        // Walk segments (all except the last, which is the leaf field)
        let mut current_entity = np.start_entity.clone();
        for segment in &np.path.segments[..np.path.segments.len() - 1] {
            // Look up the field in the current entity's cached metadata
            let Some(fields) = source_cache.get(&current_entity) else {
                if !missing.contains(&current_entity) {
                    missing.push(current_entity.clone());
                }
                break;
            };

            let Some(field_type) = fields.get(&segment.field) else {
                log::debug!(
                    "type_tracking: nav scan: field '{}' not found on '{}'",
                    segment.field,
                    current_entity,
                );
                break;
            };

            // The field must be a lookup to navigate through
            let targets = match field_type {
                FieldType::Lookup { targets, .. } => targets,
                FieldType::Simple(_) | FieldType::OptionSet { .. } => {
                    log::debug!(
                        "type_tracking: nav scan: field '{}' on '{}' is not a lookup",
                        segment.field,
                        current_entity,
                    );
                    break;
                }
            };

            // Determine the target entity to navigate to
            let next_entity = if let Some(specified) = &segment.target {
                specified.clone()
            } else if targets.len() == 1 {
                targets[0].clone()
            } else {
                log::debug!(
                    "type_tracking: nav scan: polymorphic lookup '{}' on '{}' without target specifier, targets={:?}",
                    segment.field,
                    current_entity,
                    targets,
                );
                break;
            };

            if !source_cache.contains_key(&next_entity) && !missing.contains(&next_entity) {
                missing.push(next_entity.clone());
            }

            current_entity = next_entity;
        }
    }

    missing
}
