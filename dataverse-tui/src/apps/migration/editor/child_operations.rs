//! CRUD operations for match branches, coalesce chains, and find conditions.

use rafter::prelude::*;

use crate::apps::migration::modals::TargetFieldModal;
use crate::apps::migration::modals::GuardTransformModal;
use crate::apps::migration::modals::VariableInfo;
use crate::apps::migration::repository::MigrationRepository;
use crate::apps::migration::repository::NewCoalesceChain;
use crate::apps::migration::repository::NewFindCondition;
use crate::apps::migration::repository::NewMatchBranch;
use crate::apps::migration::repository::UpdateFindCondition;
use crate::apps::migration::repository::UpdateMatchBranch;
use crate::apps::migration::types::CoalesceChain;
use crate::apps::migration::types::Condition;
use crate::apps::migration::types::Expr;
use crate::apps::migration::types::FindCondition;
use crate::apps::migration::types::MatchBranch;
use crate::apps::migration::types::SystemVar;
use crate::apps::migration::types::Transform;
use crate::apps::migration::types::TransformData;
use crate::modals::ConfirmModal;

use super::MigrationEditor;

impl MigrationEditor {
    // =========================================================================
    // Match Branch Operations
    // =========================================================================

    /// Add a new match branch to a match transform.
    pub(super) async fn add_match_branch_impl(&self, transform: &Transform, gx: &GlobalContext) {
        // Get entity mapping for source entity + variables
        let entity_mapping = self
            .entity_mappings
            .get()
            .iter()
            .find(|em| em.id == transform.entity_mapping_id)
            .cloned();

        let Some(entity_mapping) = entity_mapping else {
            log::error!("Entity mapping not found for transform");
            return;
        };

        let source_entity = entity_mapping.source_entity;
        let variables: Vec<VariableInfo> = self
            .variables
            .get()
            .iter()
            .filter(|v| v.entity_mapping_id == entity_mapping.id)
            .map(|v| VariableInfo {
                name: v.name.clone(),
                declared_type: v.declared_type.clone(),
            })
            .collect();

        // Open guard-style condition modal for the branch condition
        let default_condition = Condition::IsNull(Expr::SystemVar(SystemVar::Value));
        let modal = GuardTransformModal::new_modal(
            self.source_client.get().clone(),
            source_entity,
            variables,
            default_condition,
        );

        let Some(condition) = gx.modal(modal).await else {
            return;
        };

        // Determine order (append at end)
        let branches = self.match_branches.get();
        let order = branches
            .iter()
            .filter(|mb| mb.transform_id == transform.id)
            .count() as i32;

        let repo = gx.data::<MigrationRepository>();
        match repo
            .create_match_branch(NewMatchBranch {
                transform_id: transform.id,
                order,
                condition,
            })
            .await
        {
            Ok(_id) => {
                gx.toast(Toast::info("Branch added"));
                self.load_db_data(gx).await;
            }
            Err(e) => {
                log::error!("Failed to create match branch: {}", e);
                gx.toast(Toast::error("Failed to create branch"));
            }
        }
    }

    /// Edit a match branch's condition.
    pub(super) async fn edit_match_branch_impl(&self, branch: &MatchBranch, gx: &GlobalContext) {
        // Get entity mapping via the parent transform
        let transform = self
            .transforms
            .get()
            .iter()
            .find(|t| t.id == branch.transform_id)
            .cloned();

        let Some(transform) = transform else {
            log::error!("Parent transform not found for match branch");
            return;
        };

        let entity_mapping = self
            .entity_mappings
            .get()
            .iter()
            .find(|em| em.id == transform.entity_mapping_id)
            .cloned();

        let Some(entity_mapping) = entity_mapping else {
            log::error!("Entity mapping not found for transform");
            return;
        };

        let source_entity = entity_mapping.source_entity;
        let variables: Vec<VariableInfo> = self
            .variables
            .get()
            .iter()
            .filter(|v| v.entity_mapping_id == entity_mapping.id)
            .map(|v| VariableInfo {
                name: v.name.clone(),
                declared_type: v.declared_type.clone(),
            })
            .collect();

        let modal = GuardTransformModal::new_modal(
            self.source_client.get().clone(),
            source_entity,
            variables,
            branch.condition.clone(),
        );

        let Some(new_condition) = gx.modal(modal).await else {
            return;
        };

        let repo = gx.data::<MigrationRepository>();
        match repo
            .update_match_branch(
                branch.id,
                UpdateMatchBranch {
                    condition: Some(new_condition),
                },
            )
            .await
        {
            Ok(()) => {
                gx.toast(Toast::info("Branch updated"));
                self.load_db_data(gx).await;
            }
            Err(e) => {
                log::error!("Failed to update match branch: {}", e);
                gx.toast(Toast::error("Failed to update branch"));
            }
        }
    }

    /// Delete a match branch and its child transforms.
    pub(super) async fn delete_match_branch_impl(
        &self,
        branch: &MatchBranch,
        cx: &AppContext,
        gx: &GlobalContext,
    ) {
        // Compute next focus before deletion
        let branches = self.match_branches.get();
        let siblings: Vec<_> = branches
            .iter()
            .filter(|mb| mb.transform_id == branch.transform_id)
            .collect();
        let current_idx = siblings.iter().position(|mb| mb.id == branch.id);

        let next_focus = current_idx.and_then(|idx| {
            if idx > 0 {
                siblings
                    .get(idx - 1)
                    .map(|mb| format!("match-branch-{}", mb.id))
            } else if idx + 1 < siblings.len() {
                siblings
                    .get(idx + 1)
                    .map(|mb| format!("match-branch-{}", mb.id))
            } else {
                Some(format!("transform-{}", branch.transform_id))
            }
        });

        let confirmed = gx
            .modal(ConfirmModal::with_message("Delete this branch?"))
            .await;

        if !confirmed {
            return;
        }

        let repo = gx.data::<MigrationRepository>();
        match repo.delete_match_branch(branch.id).await {
            Ok(()) => {
                // Reorder remaining siblings
                if let Ok(remaining) = repo.get_match_branches(branch.transform_id).await {
                    let ordered_ids: Vec<i64> = remaining.iter().map(|mb| mb.id).collect();
                    let _ = repo
                        .reorder_match_branches(branch.transform_id, ordered_ids)
                        .await;
                }

                gx.toast(Toast::info("Branch deleted"));
                self.load_db_data(gx).await;

                if let Some(key) = next_focus {
                    cx.focus(format!("migration-tree-node-{}", key));
                }
            }
            Err(e) => {
                log::error!("Failed to delete match branch: {}", e);
                gx.toast(Toast::error("Failed to delete branch"));
            }
        }
    }

    /// Reorder a match branch within its transform.
    pub(super) async fn reorder_match_branch_impl(
        &self,
        branch: &MatchBranch,
        direction: i32,
        gx: &GlobalContext,
    ) {
        let branches = self.match_branches.get();
        let mut siblings: Vec<_> = branches
            .iter()
            .filter(|mb| mb.transform_id == branch.transform_id)
            .collect();
        siblings.sort_by_key(|mb| mb.order);

        let Some(current_idx) = siblings.iter().position(|mb| mb.id == branch.id) else {
            return;
        };

        let new_idx = (current_idx as i32 + direction).max(0) as usize;
        if new_idx >= siblings.len() || new_idx == current_idx {
            return;
        }

        let mut ordered_ids: Vec<i64> = siblings.iter().map(|mb| mb.id).collect();
        ordered_ids.remove(current_idx);
        ordered_ids.insert(new_idx, branch.id);

        let repo = gx.data::<MigrationRepository>();
        match repo
            .reorder_match_branches(branch.transform_id, ordered_ids)
            .await
        {
            Ok(()) => {
                self.load_db_data(gx).await;
            }
            Err(e) => {
                log::error!("Failed to reorder match branches: {}", e);
                gx.toast(Toast::error("Failed to reorder branches"));
            }
        }
    }

    // =========================================================================
    // Coalesce Chain Operations
    // =========================================================================

    /// Add a new fallback chain to a coalesce transform.
    pub(super) async fn add_coalesce_chain_impl(&self, transform: &Transform, gx: &GlobalContext) {
        let chains = self.coalesce_chains.get();
        let order = chains
            .iter()
            .filter(|cc| cc.transform_id == transform.id)
            .count() as i32;

        let repo = gx.data::<MigrationRepository>();
        match repo
            .create_coalesce_chain(NewCoalesceChain {
                transform_id: transform.id,
                order,
            })
            .await
        {
            Ok(_id) => {
                gx.toast(Toast::info("Fallback chain added"));
                self.load_db_data(gx).await;
            }
            Err(e) => {
                log::error!("Failed to create coalesce chain: {}", e);
                gx.toast(Toast::error("Failed to create fallback chain"));
            }
        }
    }

    /// Delete a coalesce chain and its child transforms.
    pub(super) async fn delete_coalesce_chain_impl(
        &self,
        chain: &CoalesceChain,
        cx: &AppContext,
        gx: &GlobalContext,
    ) {
        let chains = self.coalesce_chains.get();
        let siblings: Vec<_> = chains
            .iter()
            .filter(|cc| cc.transform_id == chain.transform_id)
            .collect();
        let current_idx = siblings.iter().position(|cc| cc.id == chain.id);

        let next_focus = current_idx.and_then(|idx| {
            if idx > 0 {
                siblings
                    .get(idx - 1)
                    .map(|cc| format!("coalesce-chain-{}", cc.id))
            } else if idx + 1 < siblings.len() {
                siblings
                    .get(idx + 1)
                    .map(|cc| format!("coalesce-chain-{}", cc.id))
            } else {
                Some(format!("transform-{}", chain.transform_id))
            }
        });

        let confirmed = gx
            .modal(ConfirmModal::with_message(
                "Delete this fallback chain and its transforms?",
            ))
            .await;

        if !confirmed {
            return;
        }

        let repo = gx.data::<MigrationRepository>();
        match repo.delete_coalesce_chain(chain.id).await {
            Ok(()) => {
                if let Ok(remaining) = repo.get_coalesce_chains(chain.transform_id).await {
                    let ordered_ids: Vec<i64> = remaining.iter().map(|cc| cc.id).collect();
                    let _ = repo
                        .reorder_coalesce_chains(chain.transform_id, ordered_ids)
                        .await;
                }

                gx.toast(Toast::info("Fallback chain deleted"));
                self.load_db_data(gx).await;

                if let Some(key) = next_focus {
                    cx.focus(format!("migration-tree-node-{}", key));
                }
            }
            Err(e) => {
                log::error!("Failed to delete coalesce chain: {}", e);
                gx.toast(Toast::error("Failed to delete fallback chain"));
            }
        }
    }

    /// Reorder a coalesce chain within its transform.
    pub(super) async fn reorder_coalesce_chain_impl(
        &self,
        chain: &CoalesceChain,
        direction: i32,
        gx: &GlobalContext,
    ) {
        let chains = self.coalesce_chains.get();
        let mut siblings: Vec<_> = chains
            .iter()
            .filter(|cc| cc.transform_id == chain.transform_id)
            .collect();
        siblings.sort_by_key(|cc| cc.order);

        let Some(current_idx) = siblings.iter().position(|cc| cc.id == chain.id) else {
            return;
        };

        let new_idx = (current_idx as i32 + direction).max(0) as usize;
        if new_idx >= siblings.len() || new_idx == current_idx {
            return;
        }

        let mut ordered_ids: Vec<i64> = siblings.iter().map(|cc| cc.id).collect();
        ordered_ids.remove(current_idx);
        ordered_ids.insert(new_idx, chain.id);

        let repo = gx.data::<MigrationRepository>();
        match repo
            .reorder_coalesce_chains(chain.transform_id, ordered_ids)
            .await
        {
            Ok(()) => {
                self.load_db_data(gx).await;
            }
            Err(e) => {
                log::error!("Failed to reorder coalesce chains: {}", e);
                gx.toast(Toast::error("Failed to reorder fallback chains"));
            }
        }
    }

    // =========================================================================
    // Find Condition Operations
    // =========================================================================

    /// Add a new find condition to a find transform.
    pub(super) async fn add_find_condition_impl(&self, transform: &Transform, gx: &GlobalContext) {
        // Get the find entity from the transform data
        let entity = match &transform.data {
            TransformData::Find { entity, .. } => entity.clone(),
            _ => {
                log::error!("add_find_condition_impl called on non-Find transform");
                return;
            }
        };

        let client = self.target_client.get();
        let Some(target_field) = gx
            .modal(TargetFieldModal::new_modal(
                client,
                entity,
                "Find Condition",
                "Select the field to match on in the find entity.",
            ))
            .await
        else {
            return;
        };

        // Determine order (append at end)
        let order = self
            .find_conditions
            .get()
            .iter()
            .filter(|fc| fc.transform_id == transform.id)
            .count() as i32;

        let repo = gx.data::<MigrationRepository>();
        match repo
            .create_find_condition(NewFindCondition {
                transform_id: transform.id,
                target_field,
                order,
            })
            .await
        {
            Ok(_id) => {
                gx.toast(Toast::info("Condition added"));
                self.load_db_data(gx).await;
            }
            Err(e) => {
                log::error!("Failed to create find condition: {}", e);
                gx.toast(Toast::error("Failed to create condition"));
            }
        }
    }

    /// Edit a find condition's target field.
    pub(super) async fn edit_find_condition_impl(&self, fc: &FindCondition, gx: &GlobalContext) {
        // Get the find entity from the parent transform
        let transform = self
            .transforms
            .get()
            .iter()
            .find(|t| t.id == fc.transform_id)
            .cloned();

        let Some(transform) = transform else {
            log::error!("Parent transform not found for find condition");
            return;
        };

        let entity = match &transform.data {
            TransformData::Find { entity, .. } => entity.clone(),
            _ => {
                log::error!("Parent transform is not a Find");
                return;
            }
        };

        let client = self.target_client.get();
        let Some(new_field) = gx
            .modal(TargetFieldModal::edit_modal(
                client,
                entity,
                "Find Condition",
                "Select the field to match on in the find entity.",
                &fc.target_field,
            ))
            .await
        else {
            return;
        };

        if new_field == fc.target_field {
            return; // No change
        }

        let repo = gx.data::<MigrationRepository>();
        match repo
            .update_find_condition(
                fc.id,
                UpdateFindCondition {
                    target_field: Some(new_field),
                },
            )
            .await
        {
            Ok(()) => {
                gx.toast(Toast::info("Condition updated"));
                self.load_db_data(gx).await;
            }
            Err(e) => {
                log::error!("Failed to update find condition: {}", e);
                gx.toast(Toast::error("Failed to update condition"));
            }
        }
    }

    /// Delete a find condition and its child transforms.
    pub(super) async fn delete_find_condition_impl(
        &self,
        fc: &FindCondition,
        cx: &AppContext,
        gx: &GlobalContext,
    ) {
        // Compute next focus before deletion
        let conditions = self.find_conditions.get();
        let siblings: Vec<_> = conditions
            .iter()
            .filter(|c| c.transform_id == fc.transform_id)
            .collect();
        let current_idx = siblings.iter().position(|c| c.id == fc.id);

        let next_focus = current_idx.and_then(|idx| {
            if idx > 0 {
                siblings
                    .get(idx - 1)
                    .map(|c| format!("find-condition-{}", c.id))
            } else if idx + 1 < siblings.len() {
                siblings
                    .get(idx + 1)
                    .map(|c| format!("find-condition-{}", c.id))
            } else {
                Some(format!("transform-{}", fc.transform_id))
            }
        });

        let confirmed = gx
            .modal(ConfirmModal::with_message("Delete this condition?"))
            .await;

        if !confirmed {
            return;
        }

        let repo = gx.data::<MigrationRepository>();
        match repo.delete_find_condition(fc.id).await {
            Ok(()) => {
                // Reorder remaining siblings
                if let Ok(remaining) = repo.get_find_conditions(fc.transform_id).await {
                    let ordered_ids: Vec<i64> = remaining.iter().map(|c| c.id).collect();
                    let _ = repo
                        .reorder_find_conditions(fc.transform_id, ordered_ids)
                        .await;
                }

                gx.toast(Toast::info("Condition deleted"));
                self.load_db_data(gx).await;

                if let Some(key) = next_focus {
                    cx.focus(format!("migration-tree-node-{}", key));
                }
            }
            Err(e) => {
                log::error!("Failed to delete find condition: {}", e);
                gx.toast(Toast::error("Failed to delete condition"));
            }
        }
    }

    /// Reorder a find condition within its transform.
    pub(super) async fn reorder_find_condition_impl(
        &self,
        fc: &FindCondition,
        direction: i32,
        gx: &GlobalContext,
    ) {
        let conditions = self.find_conditions.get();
        let mut siblings: Vec<_> = conditions
            .iter()
            .filter(|c| c.transform_id == fc.transform_id)
            .collect();
        siblings.sort_by_key(|c| c.order);

        let Some(current_idx) = siblings.iter().position(|c| c.id == fc.id) else {
            return;
        };

        let new_idx = (current_idx as i32 + direction).max(0) as usize;
        if new_idx >= siblings.len() || new_idx == current_idx {
            return;
        }

        let mut ordered_ids: Vec<i64> = siblings.iter().map(|c| c.id).collect();
        ordered_ids.remove(current_idx);
        ordered_ids.insert(new_idx, fc.id);

        let repo = gx.data::<MigrationRepository>();
        match repo
            .reorder_find_conditions(fc.transform_id, ordered_ids)
            .await
        {
            Ok(()) => {
                self.load_db_data(gx).await;
            }
            Err(e) => {
                log::error!("Failed to reorder find conditions: {}", e);
                gx.toast(Toast::error("Failed to reorder conditions"));
            }
        }
    }

}
