//! Config node edit operations (test GUIDs, passes, unmatched handling, filters, match config).

use dataverse_lib::error::Error as DataverseError;
use rafter::prelude::*;

use crate::apps::migration::modals::MatchConfigModal;
use crate::apps::migration::modals::PassesModal;
use crate::apps::migration::modals::TargetFieldModal;
use crate::apps::migration::modals::TestGuidsModal;
use crate::apps::migration::modals::UnmatchedHandlingModal;
use crate::apps::migration::repository::MigrationRepository;
use crate::apps::migration::repository::NewMatchCondition;
use crate::apps::migration::repository::Update;
use crate::apps::migration::repository::UpdateEntityMapping;
use crate::apps::migration::repository::UpdateMatchCondition;
use crate::apps::migration::types::MatchCondition;
use crate::apps::migration::types::MatchStrategy;
use crate::modals::ConfirmModal;
use crate::modals::FilterBuilderModal;
use crate::modals::LoadingModal;
use crate::widgets::filter_builder::FilterNode;

use super::MigrationEditor;

impl MigrationEditor {
    /// Edit test GUIDs for an entity mapping.
    pub(super) async fn edit_test_guids_impl(&self, entity_mapping_id: i64, gx: &GlobalContext) {
        // Find the entity mapping
        let entity_mappings = self.entity_mappings.get();
        let Some(em) = entity_mappings.iter().find(|em| em.id == entity_mapping_id) else {
            return;
        };

        // Get current test GUIDs
        let initial_guids = em.test_guids.clone().unwrap_or_default();

        // Show modal
        let Some(result) = gx
            .modal(TestGuidsModal::new_modal(entity_mapping_id, initial_guids))
            .await
        else {
            return;
        };

        // Update entity mapping
        let repo = gx.data::<MigrationRepository>();
        let update = UpdateEntityMapping {
            name: None,
            source_entity: None,
            target_entity: None,
            mode: None,
            lua_script: Update::Keep,
            match_strategy: None,
            match_find_config: None,
            match_lua_script: Update::Keep,
            no_match_fallback: None,
            orphan_strategy: None,
            create_pass_enabled: None,
            activate_pass_enabled: None,
            update_pass_enabled: None,
            delete_pass_enabled: None,
            deactivate_pass_enabled: None,
            associate_pass_enabled: None,
            disassociate_pass_enabled: None,
            source_filter: Update::Keep,
            target_filter: Update::Keep,
            test_guids: Some(result),
        };

        match repo.update_entity_mapping(entity_mapping_id, update).await {
            Ok(()) => {
                gx.toast(Toast::info("Test GUIDs updated"));
                self.load_db_data(gx).await;
            }
            Err(e) => {
                log::error!("Failed to update test GUIDs: {}", e);
                gx.toast(Toast::error("Failed to update test GUIDs"));
            }
        }
    }

    /// Edit passes for an entity mapping.
    pub(super) async fn edit_passes_impl(&self, entity_mapping_id: i64, gx: &GlobalContext) {
        // Find the entity mapping
        let entity_mappings = self.entity_mappings.get();
        let Some(em) = entity_mappings.iter().find(|em| em.id == entity_mapping_id) else {
            return;
        };

        // Show modal
        let Some(result) = gx
            .modal(PassesModal::new_modal(
                entity_mapping_id,
                em.create_pass_enabled,
                em.activate_pass_enabled,
                em.update_pass_enabled,
                em.delete_pass_enabled,
                em.deactivate_pass_enabled,
                em.associate_pass_enabled,
                em.disassociate_pass_enabled,
            ))
            .await
        else {
            return;
        };

        // Update entity mapping
        let repo = gx.data::<MigrationRepository>();
        let update = UpdateEntityMapping {
            name: None,
            source_entity: None,
            target_entity: None,
            mode: None,
            lua_script: Update::Keep,
            match_strategy: None,
            match_find_config: None,
            match_lua_script: Update::Keep,
            no_match_fallback: None,
            orphan_strategy: None,
            create_pass_enabled: Some(result.create_pass),
            activate_pass_enabled: Some(result.activate_pass),
            update_pass_enabled: Some(result.update_pass),
            delete_pass_enabled: Some(result.delete_pass),
            deactivate_pass_enabled: Some(result.deactivate_pass),
            associate_pass_enabled: Some(result.associate_pass),
            disassociate_pass_enabled: Some(result.disassociate_pass),
            source_filter: Update::Keep,
            target_filter: Update::Keep,
            test_guids: None,
        };

        match repo.update_entity_mapping(entity_mapping_id, update).await {
            Ok(()) => {
                gx.toast(Toast::info("Passes updated"));
                self.load_db_data(gx).await;
            }
            Err(e) => {
                log::error!("Failed to update passes: {}", e);
                gx.toast(Toast::error("Failed to update passes"));
            }
        }
    }

    /// Edit unmatched handling for an entity mapping.
    pub(super) async fn edit_unmatched_handling_impl(
        &self,
        entity_mapping_id: i64,
        gx: &GlobalContext,
    ) {
        // Find the entity mapping
        let entity_mappings = self.entity_mappings.get();
        let Some(em) = entity_mappings.iter().find(|em| em.id == entity_mapping_id) else {
            return;
        };

        // Show modal
        let Some(result) = gx
            .modal(UnmatchedHandlingModal::new_modal(
                entity_mapping_id,
                em.no_match_fallback,
                em.orphan_strategy,
            ))
            .await
        else {
            return;
        };

        // Update entity mapping
        let repo = gx.data::<MigrationRepository>();
        let update = UpdateEntityMapping {
            name: None,
            source_entity: None,
            target_entity: None,
            mode: None,
            lua_script: Update::Keep,
            match_strategy: None,
            match_find_config: None,
            match_lua_script: Update::Keep,
            no_match_fallback: Some(result.no_match_fallback),
            orphan_strategy: Some(result.orphan_strategy),
            create_pass_enabled: None,
            activate_pass_enabled: None,
            update_pass_enabled: None,
            delete_pass_enabled: None,
            deactivate_pass_enabled: None,
            associate_pass_enabled: None,
            disassociate_pass_enabled: None,
            source_filter: Update::Keep,
            target_filter: Update::Keep,
            test_guids: None,
        };

        match repo.update_entity_mapping(entity_mapping_id, update).await {
            Ok(()) => {
                gx.toast(Toast::info("Unmatched handling updated"));
                self.load_db_data(gx).await;
            }
            Err(e) => {
                log::error!("Failed to update unmatched handling: {}", e);
                gx.toast(Toast::error("Failed to update unmatched handling"));
            }
        }
    }

    /// Edit source filter for an entity mapping.
    pub(super) async fn edit_source_filter_impl(&self, entity_mapping_id: i64, gx: &GlobalContext) {
        let entity_mappings = self.entity_mappings.get();
        let Some(em) = entity_mappings.iter().find(|em| em.id == entity_mapping_id) else {
            return;
        };

        let entity_name = em.source_entity.clone();
        let current_filter = em.source_filter.clone();
        let client = self.source_client.get();

        // Open filter builder modal (fetches metadata internally)
        let result = gx
            .modal(FilterBuilderModal::new_modal(
                "Source Filter",
                client,
                entity_name,
                current_filter,
            ))
            .await;

        let Some(filter) = result else {
            return;
        };

        // Convert to Update enum
        let filter_update = if let FilterNode::Empty = filter {
            Update::Clear
        } else {
            Update::Set(filter)
        };

        // Update entity mapping
        let repo = gx.data::<MigrationRepository>();
        let update = UpdateEntityMapping {
            name: None,
            source_entity: None,
            target_entity: None,
            mode: None,
            lua_script: Update::Keep,
            match_strategy: None,
            match_find_config: None,
            match_lua_script: Update::Keep,
            no_match_fallback: None,
            orphan_strategy: None,
            create_pass_enabled: None,
            activate_pass_enabled: None,
            update_pass_enabled: None,
            delete_pass_enabled: None,
            deactivate_pass_enabled: None,
            associate_pass_enabled: None,
            disassociate_pass_enabled: None,
            source_filter: filter_update,
            target_filter: Update::Keep,
            test_guids: None,
        };

        match repo.update_entity_mapping(entity_mapping_id, update).await {
            Ok(()) => {
                gx.toast(Toast::info("Source filter updated"));
                self.load_db_data(gx).await;
            }
            Err(e) => {
                log::error!("Failed to update source filter: {}", e);
                gx.toast(Toast::error("Failed to update source filter"));
            }
        }
    }

    /// Edit target filter for an entity mapping.
    pub(super) async fn edit_target_filter_impl(&self, entity_mapping_id: i64, gx: &GlobalContext) {
        let entity_mappings = self.entity_mappings.get();
        let Some(em) = entity_mappings.iter().find(|em| em.id == entity_mapping_id) else {
            return;
        };

        let entity_name = em.target_entity.clone();
        let current_filter = em.target_filter.clone();
        let client = self.target_client.get();

        // Open filter builder modal (fetches metadata internally)
        let result = gx
            .modal(FilterBuilderModal::new_modal(
                "Target Filter",
                client,
                entity_name,
                current_filter,
            ))
            .await;

        let Some(filter) = result else {
            return;
        };

        // Convert to Update enum
        let filter_update = if let FilterNode::Empty = filter {
            Update::Clear
        } else {
            Update::Set(filter)
        };

        // Update entity mapping
        let repo = gx.data::<MigrationRepository>();
        let update = UpdateEntityMapping {
            name: None,
            source_entity: None,
            target_entity: None,
            mode: None,
            lua_script: Update::Keep,
            match_strategy: None,
            match_find_config: None,
            match_lua_script: Update::Keep,
            no_match_fallback: None,
            orphan_strategy: None,
            create_pass_enabled: None,
            activate_pass_enabled: None,
            update_pass_enabled: None,
            delete_pass_enabled: None,
            deactivate_pass_enabled: None,
            associate_pass_enabled: None,
            disassociate_pass_enabled: None,
            source_filter: Update::Keep,
            target_filter: filter_update,
            test_guids: None,
        };

        match repo.update_entity_mapping(entity_mapping_id, update).await {
            Ok(()) => {
                gx.toast(Toast::info("Target filter updated"));
                self.load_db_data(gx).await;
            }
            Err(e) => {
                log::error!("Failed to update target filter: {}", e);
                gx.toast(Toast::error("Failed to update target filter"));
            }
        }
    }

    // =========================================================================
    // Match Config
    // =========================================================================

    /// Edit match config for an entity mapping.
    pub(super) async fn edit_match_config_impl(&self, entity_mapping_id: i64, gx: &GlobalContext) {
        let entity_mappings = self.entity_mappings.get();
        let Some(em) = entity_mappings.iter().find(|em| em.id == entity_mapping_id) else {
            return;
        };

        let current_strategy = em.match_strategy;
        let current_lua_script = em.match_lua_script.clone();

        let Some(result) = gx
            .modal(MatchConfigModal::new_modal(current_strategy, current_lua_script.clone()))
            .await
        else {
            return;
        };

        let new_strategy = result.strategy;
        let strategy_changed = new_strategy != current_strategy;
        let script_changed = result.lua_script != current_lua_script;

        if !strategy_changed && !script_changed {
            return; // No change
        }

        // If switching away from Find, confirm deletion of conditions
        if current_strategy == MatchStrategy::Find && new_strategy != MatchStrategy::Find {
            let has_conditions = self
                .match_conditions
                .get()
                .iter()
                .any(|mc| mc.entity_mapping_id == entity_mapping_id);

            if has_conditions {
                let confirmed = gx
                    .modal(ConfirmModal::with_message(
                        "Switching away from Find will delete all match conditions. Continue?",
                    ))
                    .await;

                if !confirmed {
                    return;
                }

                // Delete all match conditions and their chains
                let repo = gx.data::<MigrationRepository>();
                if let Err(e) = repo
                    .delete_match_conditions_for_entity_mapping(entity_mapping_id)
                    .await
                {
                    log::error!("Failed to delete match conditions: {}", e);
                    gx.toast(Toast::error("Failed to delete match conditions"));
                    return;
                }
            }
        }

        // Determine match_lua_script update
        let match_lua_script = if new_strategy == MatchStrategy::Lua {
            match result.lua_script {
                Some(script) => Update::Set(script),
                None => Update::Keep,
            }
        } else {
            // Switching away from Lua — clear the script
            if current_strategy == MatchStrategy::Lua {
                Update::Clear
            } else {
                Update::Keep
            }
        };

        // Update entity mapping
        let repo = gx.data::<MigrationRepository>();
        let update = UpdateEntityMapping {
            name: None,
            source_entity: None,
            target_entity: None,
            mode: None,
            lua_script: Update::Keep,
            match_strategy: if strategy_changed {
                Some(new_strategy)
            } else {
                None
            },
            match_find_config: None,
            match_lua_script,
            no_match_fallback: None,
            orphan_strategy: None,
            create_pass_enabled: None,
            activate_pass_enabled: None,
            update_pass_enabled: None,
            delete_pass_enabled: None,
            deactivate_pass_enabled: None,
            associate_pass_enabled: None,
            disassociate_pass_enabled: None,
            source_filter: Update::Keep,
            target_filter: Update::Keep,
            test_guids: None,
        };

        match repo.update_entity_mapping(entity_mapping_id, update).await {
            Ok(()) => {
                gx.toast(Toast::info("Match config updated"));
                self.load_db_data(gx).await;
            }
            Err(e) => {
                log::error!("Failed to update match config: {}", e);
                gx.toast(Toast::error("Failed to update match config"));
            }
        }
    }

    // =========================================================================
    // Match Conditions
    // =========================================================================

    /// Fetch target entity field options for match condition autocomplete.
    async fn fetch_target_entity_fields_for_match(
        &self,
        entity_mapping_id: i64,
        gx: &GlobalContext,
    ) -> Option<Vec<(String, String)>> {
        let entity_mappings = self.entity_mappings.get();
        let em = entity_mappings
            .iter()
            .find(|em| em.id == entity_mapping_id)?;

        let client = self.target_client.get();
        let entity_name = em.target_entity.clone();
        let attributes = gx
            .modal(LoadingModal::run_with_default(
                "Loading entity fields...",
                || Err(DataverseError::Cancelled),
                async move { client.metadata().attributes(entity_name).await },
            ))
            .await;

        match attributes {
            Ok(attrs) => {
                let options: Vec<(String, String)> = attrs
                    .iter()
                    .map(|a| {
                        let display_name = a.display_name.text_or(&a.logical_name);
                        let display = if display_name == a.logical_name {
                            a.logical_name.clone()
                        } else {
                            format!("{} ({})", a.logical_name, display_name)
                        };
                        (a.logical_name.clone(), display)
                    })
                    .collect();
                Some(options)
            }
            Err(e) if e.is_cancelled() => None,
            Err(e) => {
                log::error!("Failed to fetch target entity fields: {}", e);
                gx.toast(Toast::error("Failed to fetch entity fields"));
                None
            }
        }
    }

    /// Add a match condition to an entity mapping.
    pub(super) async fn add_match_condition_impl(
        &self,
        entity_mapping_id: i64,
        gx: &GlobalContext,
    ) {
        // Verify entity mapping is in Find mode
        let entity_mappings = self.entity_mappings.get();
        let Some(em) = entity_mappings.iter().find(|em| em.id == entity_mapping_id) else {
            return;
        };

        if em.match_strategy != MatchStrategy::Find {
            gx.toast(Toast::warning(
                "Match conditions are only available in Find mode",
            ));
            return;
        }

        let client = self.target_client.get();
        let Some(target_field) = gx
            .modal(TargetFieldModal::new_modal(
                client,
                em.target_entity.clone(),
                "Match Condition",
                "Select the field to match on in the target entity.",
            ))
            .await
        else {
            return;
        };

        // Determine order (append at end)
        let order = self
            .match_conditions
            .get()
            .iter()
            .filter(|mc| mc.entity_mapping_id == entity_mapping_id)
            .count() as i32;

        let repo = gx.data::<MigrationRepository>();
        match repo
            .create_match_condition(NewMatchCondition {
                entity_mapping_id,
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
                log::error!("Failed to create match condition: {}", e);
                gx.toast(Toast::error("Failed to create condition"));
            }
        }
    }

    /// Edit a match condition's target field.
    pub(super) async fn edit_match_condition_impl(&self, mc: &MatchCondition, gx: &GlobalContext) {
        let entity_mappings = self.entity_mappings.get();
        let Some(em) = entity_mappings
            .iter()
            .find(|em| em.id == mc.entity_mapping_id)
        else {
            return;
        };

        let client = self.target_client.get();
        let Some(new_field) = gx
            .modal(TargetFieldModal::edit_modal(
                client,
                em.target_entity.clone(),
                "Match Condition",
                "Select the field to match on in the target entity.",
                &mc.target_field,
            ))
            .await
        else {
            return;
        };

        if new_field == mc.target_field {
            return; // No change
        }

        let repo = gx.data::<MigrationRepository>();
        match repo
            .update_match_condition(
                mc.id,
                UpdateMatchCondition {
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
                log::error!("Failed to update match condition: {}", e);
                gx.toast(Toast::error("Failed to update condition"));
            }
        }
    }

    /// Delete a match condition and its child transforms.
    pub(super) async fn delete_match_condition_impl(
        &self,
        mc: &MatchCondition,
        cx: &AppContext,
        gx: &GlobalContext,
    ) {
        // Compute next focus before deletion
        let conditions = self.match_conditions.get();
        let siblings: Vec<_> = conditions
            .iter()
            .filter(|c| c.entity_mapping_id == mc.entity_mapping_id)
            .collect();
        let current_idx = siblings.iter().position(|c| c.id == mc.id);

        let next_focus = current_idx.and_then(|idx| {
            if idx > 0 {
                siblings
                    .get(idx - 1)
                    .map(|c| format!("match-condition-{}", c.id))
            } else if idx + 1 < siblings.len() {
                siblings
                    .get(idx + 1)
                    .map(|c| format!("match-condition-{}", c.id))
            } else {
                Some(format!("match-config-{}", mc.entity_mapping_id))
            }
        });

        let confirmed = gx
            .modal(ConfirmModal::with_message("Delete this condition?"))
            .await;

        if !confirmed {
            return;
        }

        let repo = gx.data::<MigrationRepository>();
        match repo.delete_match_condition(mc.id).await {
            Ok(()) => {
                // Reorder remaining siblings
                if let Ok(remaining) = repo.get_match_conditions(mc.entity_mapping_id).await {
                    let ordered_ids: Vec<i64> = remaining.iter().map(|c| c.id).collect();
                    let _ = repo
                        .reorder_match_conditions(mc.entity_mapping_id, ordered_ids)
                        .await;
                }

                gx.toast(Toast::info("Condition deleted"));
                self.load_db_data(gx).await;

                if let Some(key) = next_focus {
                    cx.focus(format!("migration-tree-node-{}", key));
                }
            }
            Err(e) => {
                log::error!("Failed to delete match condition: {}", e);
                gx.toast(Toast::error("Failed to delete condition"));
            }
        }
    }

    /// Reorder a match condition within its entity mapping.
    pub(super) async fn reorder_match_condition_impl(
        &self,
        mc: &MatchCondition,
        direction: i32,
        gx: &GlobalContext,
    ) {
        let conditions = self.match_conditions.get();
        let mut siblings: Vec<_> = conditions
            .iter()
            .filter(|c| c.entity_mapping_id == mc.entity_mapping_id)
            .collect();
        siblings.sort_by_key(|c| c.order);

        let Some(current_idx) = siblings.iter().position(|c| c.id == mc.id) else {
            return;
        };

        let new_idx = (current_idx as i32 + direction).max(0) as usize;
        if new_idx >= siblings.len() || new_idx == current_idx {
            return;
        }

        let mut ordered_ids: Vec<i64> = siblings.iter().map(|c| c.id).collect();
        ordered_ids.swap(current_idx, new_idx);

        let repo = gx.data::<MigrationRepository>();
        match repo
            .reorder_match_conditions(mc.entity_mapping_id, ordered_ids)
            .await
        {
            Ok(()) => {
                self.load_db_data(gx).await;
            }
            Err(e) => {
                log::error!("Failed to reorder match conditions: {}", e);
                gx.toast(Toast::error("Failed to reorder conditions"));
            }
        }
    }
}
