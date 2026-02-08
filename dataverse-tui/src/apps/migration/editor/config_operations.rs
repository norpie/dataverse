//! Config node edit operations (test GUIDs, passes, unmatched handling, filters).

use dataverse_lib::error::Error as DataverseError;
use rafter::prelude::*;

use crate::apps::migration::modals::PassesModal;
use crate::apps::migration::modals::TestGuidsModal;
use crate::apps::migration::modals::UnmatchedHandlingModal;
use crate::apps::migration::repository::MigrationRepository;
use crate::apps::migration::repository::UpdateEntityMapping;
use crate::modals::FilterBuilderModal;
use crate::modals::LoadingModal;
use crate::widgets::filter_builder::FilterNode;

use super::MigrationEditor;

impl MigrationEditor {
    /// Edit test GUIDs for an entity mapping.
    pub(super) async fn edit_test_guids_impl(&self, entity_mapping_id: i64, gx: &GlobalContext) {
        // Find the entity mapping
        let entity_mappings = self.entity_mappings.get();
        let Some(em) = entity_mappings
            .iter()
            .find(|em| em.id == entity_mapping_id)
        else {
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
            lua_script: crate::apps::migration::repository::Update::Keep,
            match_strategy: None,
            match_find_config: None,
            no_match_fallback: None,
            orphan_strategy: None,
            create_pass_enabled: None,
            update_pass_enabled: None,
            delete_pass_enabled: None,
            deactivate_pass_enabled: None,
            associate_pass_enabled: None,
            disassociate_pass_enabled: None,
            source_filter: None,
            target_filter: None,
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
        let Some(em) = entity_mappings
            .iter()
            .find(|em| em.id == entity_mapping_id)
        else {
            return;
        };

        // Show modal
        let Some(result) = gx
            .modal(PassesModal::new_modal(
                entity_mapping_id,
                em.create_pass_enabled,
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
            lua_script: crate::apps::migration::repository::Update::Keep,
            match_strategy: None,
            match_find_config: None,
            no_match_fallback: None,
            orphan_strategy: None,
            create_pass_enabled: Some(result.create_pass),
            update_pass_enabled: Some(result.update_pass),
            delete_pass_enabled: Some(result.delete_pass),
            deactivate_pass_enabled: Some(result.deactivate_pass),
            associate_pass_enabled: Some(result.associate_pass),
            disassociate_pass_enabled: Some(result.disassociate_pass),
            source_filter: None,
            target_filter: None,
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
        let Some(em) = entity_mappings
            .iter()
            .find(|em| em.id == entity_mapping_id)
        else {
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
            lua_script: crate::apps::migration::repository::Update::Keep,
            match_strategy: None,
            match_find_config: None,
            no_match_fallback: Some(result.no_match_fallback),
            orphan_strategy: Some(result.orphan_strategy),
            create_pass_enabled: None,
            update_pass_enabled: None,
            delete_pass_enabled: None,
            deactivate_pass_enabled: None,
            associate_pass_enabled: None,
            disassociate_pass_enabled: None,
            source_filter: None,
            target_filter: None,
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
    pub(super) async fn edit_source_filter_impl(
        &self,
        entity_mapping_id: i64,
        gx: &GlobalContext,
    ) {
        let entity_mappings = self.entity_mappings.get();
        let Some(em) = entity_mappings.iter().find(|em| em.id == entity_mapping_id) else {
            return;
        };

        let entity_name = em.source_entity.clone();
        let current_filter = em.source_filter.clone();

        // Fetch attributes for the source entity
        let client = self.source_client.get();
        let entity_name_clone = entity_name.clone();
        let attributes = gx
            .modal(LoadingModal::run_with_default(
                "Loading entity metadata...",
                || Err(DataverseError::Cancelled),
                async move { client.metadata().attributes(entity_name_clone).await },
            ))
            .await;

        let attributes = match attributes {
            Ok(attrs) => attrs,
            Err(e) if e.is_cancelled() => return,
            Err(e) => {
                log::error!("Failed to fetch attributes for {}: {}", entity_name, e);
                gx.toast(Toast::error("Failed to fetch entity metadata"));
                return;
            }
        };

        // Build options for autocomplete
        let options: Vec<(String, String)> = attributes
            .iter()
            .map(|a| {
                let display = a.display_name.text_or(&a.logical_name).to_string();
                (a.logical_name.clone(), display)
            })
            .collect();

        // Open filter builder modal
        let result = gx
            .modal(FilterBuilderModal::new_modal(
                "Source Filter",
                options,
                attributes,
                current_filter,
            ))
            .await;

        let Some(filter) = result else {
            return;
        };

        // Convert Empty to None for storage
        let filter_to_store = if let FilterNode::Empty = filter {
            None
        } else {
            Some(filter)
        };

        // Update entity mapping
        let repo = gx.data::<MigrationRepository>();
        let update = UpdateEntityMapping {
            name: None,
            source_entity: None,
            target_entity: None,
            mode: None,
            lua_script: crate::apps::migration::repository::Update::Keep,
            match_strategy: None,
            match_find_config: None,
            no_match_fallback: None,
            orphan_strategy: None,
            create_pass_enabled: None,
            update_pass_enabled: None,
            delete_pass_enabled: None,
            deactivate_pass_enabled: None,
            associate_pass_enabled: None,
            disassociate_pass_enabled: None,
            source_filter: filter_to_store,
            target_filter: None,
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
    pub(super) async fn edit_target_filter_impl(
        &self,
        entity_mapping_id: i64,
        gx: &GlobalContext,
    ) {
        let entity_mappings = self.entity_mappings.get();
        let Some(em) = entity_mappings.iter().find(|em| em.id == entity_mapping_id) else {
            return;
        };

        let entity_name = em.target_entity.clone();
        let current_filter = em.target_filter.clone();

        // Fetch attributes for the target entity
        let client = self.target_client.get();
        let entity_name_clone = entity_name.clone();
        let attributes = gx
            .modal(LoadingModal::run_with_default(
                "Loading entity metadata...",
                || Err(DataverseError::Cancelled),
                async move { client.metadata().attributes(entity_name_clone).await },
            ))
            .await;

        let attributes = match attributes {
            Ok(attrs) => attrs,
            Err(e) if e.is_cancelled() => return,
            Err(e) => {
                log::error!("Failed to fetch attributes for {}: {}", entity_name, e);
                gx.toast(Toast::error("Failed to fetch entity metadata"));
                return;
            }
        };

        // Build options for autocomplete
        let options: Vec<(String, String)> = attributes
            .iter()
            .map(|a| {
                let display = a.display_name.text_or(&a.logical_name).to_string();
                (a.logical_name.clone(), display)
            })
            .collect();

        // Open filter builder modal
        let result = gx
            .modal(FilterBuilderModal::new_modal(
                "Target Filter",
                options,
                attributes,
                current_filter,
            ))
            .await;

        let Some(filter) = result else {
            return;
        };

        // Convert Empty to None for storage
        let filter_to_store = if let FilterNode::Empty = filter {
            None
        } else {
            Some(filter)
        };

        // Update entity mapping
        let repo = gx.data::<MigrationRepository>();
        let update = UpdateEntityMapping {
            name: None,
            source_entity: None,
            target_entity: None,
            mode: None,
            lua_script: crate::apps::migration::repository::Update::Keep,
            match_strategy: None,
            match_find_config: None,
            no_match_fallback: None,
            orphan_strategy: None,
            create_pass_enabled: None,
            update_pass_enabled: None,
            delete_pass_enabled: None,
            deactivate_pass_enabled: None,
            associate_pass_enabled: None,
            disassociate_pass_enabled: None,
            source_filter: None,
            target_filter: filter_to_store,
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
}
