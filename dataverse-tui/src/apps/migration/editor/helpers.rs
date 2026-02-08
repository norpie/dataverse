//! Internal helper functions for the migration editor.

use dataverse_lib::model::metadata::AttributeType;
use dataverse_lib::model::FieldType;
use dataverse_lib::DataverseClient;
use rafter::prelude::*;

use crate::apps::migration::repository::MigrationRepository;
use crate::apps::migration::types::TransformData;
use crate::apps::migration::validation::parse_path;
use crate::apps::migration::validation::FieldPath;
use crate::apps::migration::validation::PathExpr;
use super::tree::FieldTypeCache;
use super::tree::MigrationTreeNode;
use super::MigrationEditor;

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

/// A dotted copy path paired with the entity mapping it belongs to.
pub(super) struct DottedCopyPath {
    entity_mapping_id: i64,
    path: FieldPath,
}

/// Collect all dotted copy paths (paths with 2+ segments) from transforms.
pub(super) fn collect_dotted_copy_paths(transforms: &[crate::apps::migration::types::Transform]) -> Vec<DottedCopyPath> {
    let mut paths = Vec::new();

    for t in transforms {
        if let TransformData::Copy { path } = &t.data {
            // Skip variables and system vars
            if path.starts_with('$') || path.starts_with('#') {
                continue;
            }
            // Only interested in dotted paths (2+ segments)
            if !path.contains('.') {
                continue;
            }
            if let Ok(PathExpr::Field(field_path)) = parse_path(path) {
                if field_path.segments.len() >= 2 {
                    paths.push(DottedCopyPath {
                        entity_mapping_id: t.entity_mapping_id,
                        path: field_path,
                    });
                }
            }
        }
    }

    paths
}

/// Discover navigation entities that are not yet in the source cache.
///
/// Walks each dotted path segment-by-segment using the cached metadata. For each
/// lookup segment, collects the target entity names. Returns entities that are
/// not yet in the cache.
pub(super) fn discover_navigation_entities(
    dotted_paths: &[DottedCopyPath],
    entity_mappings: &[crate::apps::migration::types::EntityMapping],
    source_cache: &FieldTypeCache,
) -> Vec<String> {
    let mut missing: Vec<String> = Vec::new();

    for dcp in dotted_paths {
        // Find the source entity for this transform's entity mapping
        let source_entity = entity_mappings
            .iter()
            .find(|em| em.id == dcp.entity_mapping_id)
            .map(|em| em.source_entity.as_str())
            .unwrap_or("");

        if source_entity.is_empty() {
            continue;
        }

        // Walk segments (all except the last, which is the leaf field)
        let mut current_entity = source_entity.to_string();
        for segment in &dcp.path.segments[..dcp.path.segments.len() - 1] {
            // Look up the field in the current entity's cached metadata
            let Some(fields) = source_cache.get(&current_entity) else {
                // Entity not yet cached — it will be discovered on the next iteration
                // after it's been fetched.
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
                // Polymorphic lookup with explicit target: ownerid[systemuser]
                specified.clone()
            } else if targets.len() == 1 {
                // Single-target lookup
                targets[0].clone()
            } else {
                // Polymorphic lookup without explicit target — can't navigate
                log::debug!(
                    "type_tracking: nav scan: polymorphic lookup '{}' on '{}' without target specifier, targets={:?}",
                    segment.field,
                    current_entity,
                    targets,
                );
                break;
            };

            // If the target entity is not yet cached, mark it as missing
            if !source_cache.contains_key(&next_entity) && !missing.contains(&next_entity) {
                missing.push(next_entity.clone());
            }

            current_entity = next_entity;
        }
    }

    missing
}
