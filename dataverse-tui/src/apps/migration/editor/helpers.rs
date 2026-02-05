//! Internal helper functions for the migration editor.

use rafter::prelude::*;

use crate::apps::migration::repository::MigrationRepository;
use crate::apps::migration::types::EntityMapping;
use crate::apps::migration::types::FieldMapping;
use crate::apps::migration::types::Phase;
use crate::apps::migration::types::Variable;

use super::tree::build_tree_nodes;
use super::tree::MigrationTreeNode;
use super::MigrationEditor;

impl MigrationEditor {
    /// Refresh all data from the repository.
    pub(super) async fn refresh_data(&self, gx: &GlobalContext) {
        let migration_id = self.migration.get().id;
        let repo = gx.data::<MigrationRepository>();

        // Reload phases
        if let Ok(phases) = repo.get_phases(migration_id).await {
            self.phases.set(phases);
        }

        // Reload entity mappings
        let phases = self.phases.get();
        let mut all_mappings = Vec::new();
        for phase in &phases {
            if let Ok(mappings) = repo.get_entity_mappings(phase.id).await {
                all_mappings.extend(mappings);
            }
        }
        self.entity_mappings.set(all_mappings);

        // Reload variables and field mappings
        let entity_mappings = self.entity_mappings.get();
        let mut all_variables = Vec::new();
        let mut all_field_mappings = Vec::new();
        for em in &entity_mappings {
            if let Ok(vars) = repo.get_variables(em.id).await {
                all_variables.extend(vars);
            }
            if let Ok(fms) = repo.get_field_mappings(em.id).await {
                all_field_mappings.extend(fms);
            }
        }
        self.variables.set(all_variables);
        self.field_mappings.set(all_field_mappings);

        self.rebuild_tree();
    }

    /// Rebuild the tree from current data.
    pub(super) fn rebuild_tree(&self) {
        let phases = self.phases.get();
        let entity_mappings = self.entity_mappings.get();
        let variables = self.variables.get();
        let field_mappings = self.field_mappings.get();

        let nodes = build_tree_nodes(phases, entity_mappings, variables, field_mappings);
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
