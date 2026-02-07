//! Internal helper functions for the migration editor.

use dataverse_lib::model::FieldType;
use dataverse_lib::DataverseClient;
use rafter::prelude::*;

use crate::apps::migration::repository::MigrationRepository;
use crate::apps::migration::types::TransformData;
use crate::apps::migration::validation::parse_path;
use crate::apps::migration::validation::FieldPath;
use crate::apps::migration::validation::PathExpr;
use crate::modals::LoadingModal;

use super::tree::build_tree_nodes;
use super::tree::FieldTypeCache;
use super::tree::MigrationTreeNode;
use super::MigrationEditor;

impl MigrationEditor {
    /// Refresh all data from the repository.
    /// Uses bulk queries to avoid N+1 (8 queries total).
    pub(super) async fn refresh_data(&self, gx: &GlobalContext) {
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

        // Fetch metadata for any new source entities not yet cached
        self.fetch_missing_metadata(gx).await;

        self.rebuild_tree();
    }

    /// Fetch entity metadata for source and target entities not yet in their caches.
    ///
    /// Also pre-scans dotted copy paths to discover navigation entities (entities
    /// referenced via lookup traversal, e.g., `parentaccountid.name` navigates to
    /// the `account` entity). These are fetched iteratively until all reachable
    /// entities are cached.
    pub(super) async fn fetch_missing_metadata(&self, gx: &GlobalContext) {
        let entity_mappings = self.entity_mappings.get();
        let current_source_cache = self.field_type_cache.get();
        let current_target_cache = self.target_field_cache.get();

        // Collect unique source entities not yet cached
        let mut missing_source: Vec<String> = Vec::new();
        for em in entity_mappings.iter() {
            if !em.source_entity.is_empty()
                && !current_source_cache.contains_key(&em.source_entity)
                && !missing_source.contains(&em.source_entity)
            {
                missing_source.push(em.source_entity.clone());
            }
        }

        // Collect unique target entities not yet cached
        let mut missing_target: Vec<String> = Vec::new();
        for em in entity_mappings.iter() {
            if !em.target_entity.is_empty()
                && !current_target_cache.contains_key(&em.target_entity)
                && !missing_target.contains(&em.target_entity)
            {
                missing_target.push(em.target_entity.clone());
            }
        }

        if missing_source.is_empty() && missing_target.is_empty() {
            return;
        }

        // Fetch source entity metadata
        if !missing_source.is_empty() {
            log::debug!(
                "type_tracking: fetching metadata for {} source entities: {:?}",
                missing_source.len(),
                missing_source,
            );

            let client = self.source_client.get().clone();
            let result: FieldTypeCache = gx
                .modal(LoadingModal::run(
                    "Loading source entity metadata...",
                    fetch_entity_field_types(client, missing_source),
                ))
                .await;

            self.field_type_cache.update(|cache| {
                for (entity, fields) in result {
                    cache.insert(entity, fields);
                }
            });
        }

        // Pre-scan dotted copy paths to discover navigation entities.
        // Iteratively fetch metadata for entities reached via lookup traversal
        // until no new entities are discovered.
        let transforms = self.transforms.get();
        let dotted_paths = collect_dotted_copy_paths(&transforms);
        if !dotted_paths.is_empty() {
            log::debug!(
                "type_tracking: found {} dotted copy paths for navigation scanning",
                dotted_paths.len(),
            );

            loop {
                let source_cache = self.field_type_cache.get();
                let nav_entities =
                    discover_navigation_entities(&dotted_paths, &entity_mappings, &source_cache);

                if nav_entities.is_empty() {
                    break;
                }

                log::debug!(
                    "type_tracking: fetching metadata for {} navigation entities: {:?}",
                    nav_entities.len(),
                    nav_entities,
                );

                let client = self.source_client.get().clone();
                let result: FieldTypeCache = gx
                    .modal(LoadingModal::run(
                        "Loading navigation entity metadata...",
                        fetch_entity_field_types(client, nav_entities),
                    ))
                    .await;

                let fetched_any = !result.is_empty();
                self.field_type_cache.update(|cache| {
                    for (entity, fields) in result {
                        cache.insert(entity, fields);
                    }
                });

                // If we didn't successfully fetch any new entities, stop to avoid
                // infinite loop (e.g., metadata fetch failures).
                if !fetched_any {
                    break;
                }
            }
        }

        // Fetch target entity metadata
        if !missing_target.is_empty() {
            log::debug!(
                "type_tracking: fetching metadata for {} target entities: {:?}",
                missing_target.len(),
                missing_target,
            );

            let client = self.target_client.get().clone();
            let result: FieldTypeCache = gx
                .modal(LoadingModal::run(
                    "Loading target entity metadata...",
                    fetch_entity_field_types(client, missing_target),
                ))
                .await;

            self.target_field_cache.update(|cache| {
                for (entity, fields) in result {
                    cache.insert(entity, fields);
                }
            });
        }
    }

    /// Rebuild the tree from current data.
    pub(super) fn rebuild_tree(&self) {
        let phases = self.phases.get();
        let entity_mappings = self.entity_mappings.get();
        let variables = self.variables.get();
        let field_mappings = self.field_mappings.get();
        let transforms = self.transforms.get();
        let match_branches = self.match_branches.get();
        let coalesce_chains = self.coalesce_chains.get();
        let find_conditions = self.find_conditions.get();
        let field_type_cache = self.field_type_cache.get();
        let target_field_cache = self.target_field_cache.get();

        let (nodes, type_tracking) = build_tree_nodes(
            phases,
            entity_mappings,
            variables,
            field_mappings,
            transforms,
            match_branches,
            coalesce_chains,
            find_conditions,
            &field_type_cache,
            &target_field_cache,
        );
        self.type_tracking.set(type_tracking);
        self.tree_state.update(|s| {
            s.set_roots(nodes);
            s.expand_all();
        });
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
async fn fetch_entity_field_types(
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
                    .map(|attr| (attr.logical_name.clone(), FieldType::from(attr)))
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

/// A dotted copy path paired with the entity mapping it belongs to.
struct DottedCopyPath {
    entity_mapping_id: i64,
    path: FieldPath,
}

/// Collect all dotted copy paths (paths with 2+ segments) from transforms.
fn collect_dotted_copy_paths(transforms: &[crate::apps::migration::types::Transform]) -> Vec<DottedCopyPath> {
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
fn discover_navigation_entities(
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
                FieldType::Simple(_) => {
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
