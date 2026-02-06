//! Internal helper functions for the migration editor.

use std::collections::HashMap;

use dataverse_lib::model::metadata::AttributeType;
use dataverse_lib::DataverseClient;
use rafter::prelude::*;

use crate::apps::migration::repository::MigrationRepository;
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

    /// Fetch entity metadata for source entities not yet in the cache.
    pub(super) async fn fetch_missing_metadata(&self, gx: &GlobalContext) {
        let entity_mappings = self.entity_mappings.get();
        let current_cache = self.field_type_cache.get();

        // Collect unique source entities not yet cached
        let mut missing: Vec<String> = Vec::new();
        for em in entity_mappings.iter() {
            if !em.source_entity.is_empty()
                && !current_cache.contains_key(&em.source_entity)
                && !missing.contains(&em.source_entity)
            {
                missing.push(em.source_entity.clone());
            }
        }

        if missing.is_empty() {
            return;
        }

        log::debug!(
            "type_tracking: fetching metadata for {} source entities: {:?}",
            missing.len(),
            missing,
        );

        let client = self.source_client.get().clone();
        let result: FieldTypeCache = gx
            .modal(LoadingModal::run(
                "Loading entity metadata...",
                fetch_entity_field_types(client, missing),
            ))
            .await;

        // Merge into existing cache
        self.field_type_cache.update(|cache| {
            for (entity, fields) in result {
                cache.insert(entity, fields);
            }
        });
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
                let fields: HashMap<String, AttributeType> = metadata
                    .attributes
                    .into_iter()
                    .map(|attr| (attr.logical_name, attr.attribute_type))
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
