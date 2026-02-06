//! Internal helper functions for the migration editor.

use rafter::prelude::*;

use crate::apps::migration::repository::MigrationRepository;

use super::tree::build_tree_nodes;
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

        self.rebuild_tree();
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

        let (nodes, type_tracking) = build_tree_nodes(
            phases,
            entity_mappings,
            variables,
            field_mappings,
            transforms,
            match_branches,
            coalesce_chains,
            find_conditions,
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
