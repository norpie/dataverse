//! Entity mapping add/edit/delete operations.

use rafter::prelude::*;

use crate::apps::migration::modals::EditEntityMappingModal;
use crate::apps::migration::repository::MigrationRepository;
use crate::apps::migration::repository::NewEntityMapping;
use crate::apps::migration::repository::Update;
use crate::apps::migration::repository::UpdateEntityMapping;
use crate::apps::migration::types::EntityMapping;
use crate::apps::migration::types::MatchStrategy;
use crate::apps::migration::types::Mode;
use crate::apps::migration::types::NoMatchFallback;
use crate::apps::migration::types::OrphanStrategy;
use crate::modals::parallel_load;

use super::MigrationEditor;

impl MigrationEditor {
    /// Reorder an entity mapping within its phase.
    pub(super) async fn reorder_entity_mapping_impl(
        &self,
        entity_mapping_id: i64,
        phase_id: i64,
        direction: i32,
        gx: &GlobalContext,
    ) {
        let entity_mappings = self.entity_mappings.get();
        let mut siblings: Vec<_> = entity_mappings
            .iter()
            .filter(|em| em.phase_id == phase_id)
            .collect();
        siblings.sort_by_key(|em| em.order);

        let Some(current_idx) = siblings.iter().position(|em| em.id == entity_mapping_id) else {
            return;
        };

        let new_idx = (current_idx as i32 + direction).max(0) as usize;
        if new_idx >= siblings.len() || new_idx == current_idx {
            return;
        }

        let mut ordered_ids: Vec<i64> = siblings.iter().map(|em| em.id).collect();
        ordered_ids.remove(current_idx);
        ordered_ids.insert(new_idx, entity_mapping_id);

        let repo = gx.data::<MigrationRepository>();
        match repo.reorder_entity_mappings(phase_id, ordered_ids).await {
            Ok(()) => {
                self.load_db_data(gx).await;
            }
            Err(e) => {
                log::error!("Failed to reorder entity mappings: {}", e);
                gx.toast(Toast::error("Failed to reorder entity mappings"));
            }
        }
    }

    /// Add a new entity mapping to a phase.
    pub(super) async fn add_entity_mapping_impl(&self, phase_id: i64, gx: &GlobalContext) {
        // Fetch entity lists from both environments in parallel
        let source_client = self.source_client.get();
        let target_client = self.target_client.get();

        let (source_result, target_result) = parallel_load!(gx, {
            "Loading source entities" => async move {
                source_client
                    .metadata()
                    .all_entities()
                    .await
                    .map(|entities| {
                        entities.into_iter().map(|e| e.logical_name).collect::<Vec<_>>()
                    })
            },
            "Loading target entities" => async move {
                target_client
                    .metadata()
                    .all_entities()
                    .await
                    .map(|entities| {
                        entities.into_iter().map(|e| e.logical_name).collect::<Vec<_>>()
                    })
            },
        });

        let source_entities = match source_result {
            Ok(Ok(entities)) => entities,
            Ok(Err(e)) => {
                log::error!("Failed to fetch source entities: {}", e);
                gx.toast(Toast::error("Failed to fetch source entities"));
                return;
            }
            Err(e) => {
                log::warn!("Source entities load cancelled: {e}");
                gx.toast(Toast::error("Failed to load source entities"));
                return;
            }
        };

        let target_entities = match target_result {
            Ok(Ok(entities)) => entities,
            Ok(Err(e)) => {
                log::error!("Failed to fetch target entities: {}", e);
                gx.toast(Toast::error("Failed to fetch target entities"));
                return;
            }
            Err(e) => {
                log::warn!("Target entities load cancelled: {e}");
                gx.toast(Toast::error("Failed to load target entities"));
                return;
            }
        };

        // Show modal
        let Some(result) = gx
            .modal(EditEntityMappingModal::new_mapping(
                source_entities,
                target_entities,
            ))
            .await
        else {
            return;
        };

        // Create entity mapping
        let repo = gx.data::<MigrationRepository>();
        let order = self
            .entity_mappings
            .get()
            .iter()
            .filter(|em| em.phase_id == phase_id)
            .count() as i32;

        let new_mapping = match result {
            crate::apps::migration::modals::EntityMappingResult::Declarative {
                name,
                source_entity,
                target_entity,
            } => NewEntityMapping {
                phase_id,
                order,
                name,
                source_entity,
                target_entity,
                mode: Mode::Declarative,
                lua_script: None,
                match_strategy: MatchStrategy::SameId,
                match_find_config: None,
                match_lua_script: None,
                no_match_fallback: NoMatchFallback::Create,
                orphan_strategy: OrphanStrategy::Ignore,
                create_pass_enabled: true,
                activate_pass_enabled: true,
                update_pass_enabled: true,
                delete_pass_enabled: true,
                deactivate_pass_enabled: true,
                associate_pass_enabled: true,
                disassociate_pass_enabled: true,
                source_filter: None,
                target_filter: None,
                test_guids: None,
            },
            crate::apps::migration::modals::EntityMappingResult::Lua { name, lua_script } => {
                NewEntityMapping {
                    phase_id,
                    order,
                    name,
                    source_entity: String::new(),
                    target_entity: String::new(),
                    mode: Mode::Lua,
                    lua_script: Some(lua_script),
                    match_strategy: MatchStrategy::SameId,
                    match_find_config: None,
                    match_lua_script: None,
                    no_match_fallback: NoMatchFallback::Create,
                    orphan_strategy: OrphanStrategy::Ignore,
                    create_pass_enabled: true,
                    activate_pass_enabled: true,
                    update_pass_enabled: true,
                    delete_pass_enabled: true,
                    deactivate_pass_enabled: true,
                    associate_pass_enabled: true,
                    disassociate_pass_enabled: true,
                    source_filter: None,
                    target_filter: None,
                    test_guids: None,
                }
            }
        };

        match repo.create_entity_mapping(new_mapping).await {
            Ok(_id) => {
                gx.toast(Toast::info("Entity mapping created"));
                self.load_db_data(gx).await;
            }
            Err(e) => {
                log::error!("Failed to create entity mapping: {}", e);
                gx.toast(Toast::error("Failed to create entity mapping"));
            }
        }
    }

    /// Edit an existing entity mapping.
    pub(super) async fn edit_entity_mapping_impl(&self, em: &EntityMapping, gx: &GlobalContext) {
        // Fetch entity lists from both environments in parallel
        let source_client = self.source_client.get();
        let target_client = self.target_client.get();

        let (source_result, target_result) = parallel_load!(gx, {
            "Loading source entities" => async move {
                source_client
                    .metadata()
                    .all_entities()
                    .await
                    .map(|entities| {
                        entities.into_iter().map(|e| e.logical_name).collect::<Vec<_>>()
                    })
            },
            "Loading target entities" => async move {
                target_client
                    .metadata()
                    .all_entities()
                    .await
                    .map(|entities| {
                        entities.into_iter().map(|e| e.logical_name).collect::<Vec<_>>()
                    })
            },
        });

        let source_entities = match source_result {
            Ok(Ok(entities)) => entities,
            Ok(Err(e)) => {
                log::error!("Failed to fetch source entities: {}", e);
                gx.toast(Toast::error("Failed to fetch source entities"));
                return;
            }
            Err(e) => {
                log::warn!("Source entities load cancelled: {e}");
                gx.toast(Toast::error("Failed to load source entities"));
                return;
            }
        };

        let target_entities = match target_result {
            Ok(Ok(entities)) => entities,
            Ok(Err(e)) => {
                log::error!("Failed to fetch target entities: {}", e);
                gx.toast(Toast::error("Failed to fetch target entities"));
                return;
            }
            Err(e) => {
                log::warn!("Target entities load cancelled: {e}");
                gx.toast(Toast::error("Failed to load target entities"));
                return;
            }
        };

        let Some(result) = gx
            .modal(EditEntityMappingModal::edit_mapping(
                em,
                source_entities,
                target_entities,
            ))
            .await
        else {
            return;
        };

        let repo = gx.data::<MigrationRepository>();
        let update = match result {
            crate::apps::migration::modals::EntityMappingResult::Declarative {
                name,
                source_entity,
                target_entity,
            } => UpdateEntityMapping {
                name: Some(name),
                source_entity: Some(source_entity),
                target_entity: Some(target_entity),
                mode: Some(Mode::Declarative),
                lua_script: crate::apps::migration::repository::Update::Clear,
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
                test_guids: None,
            },
            crate::apps::migration::modals::EntityMappingResult::Lua { name, lua_script } => {
                UpdateEntityMapping {
                    name: Some(name),
                    source_entity: Some(String::new()),
                    target_entity: Some(String::new()),
                    mode: Some(Mode::Lua),
                    lua_script: crate::apps::migration::repository::Update::Set(lua_script),
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
                    test_guids: None,
                }
            }
        };

        match repo.update_entity_mapping(em.id, update).await {
            Ok(()) => {
                self.load_db_data(gx).await;
            }
            Err(e) => {
                log::error!("Failed to update entity mapping: {}", e);
                gx.toast(Toast::error("Failed to update entity mapping"));
            }
        }
    }

    /// Delete an entity mapping.
    pub(super) async fn delete_entity_mapping_impl(
        &self,
        entity_id: i64,
        cx: &AppContext,
        gx: &GlobalContext,
    ) {
        // Find parent phase and sibling info before deletion
        let entity_mappings = self.entity_mappings.get();
        let current = entity_mappings.iter().find(|em| em.id == entity_id);

        let (_phase_id, next_focus) = if let Some(em) = current {
            let phase_id = em.phase_id;
            let siblings: Vec<_> = entity_mappings
                .iter()
                .filter(|e| e.phase_id == phase_id)
                .collect();
            let current_idx = siblings.iter().position(|e| e.id == entity_id);

            let next = current_idx.and_then(|idx| {
                // Try previous sibling, then next sibling, then parent phase
                if idx > 0 {
                    siblings.get(idx - 1).map(|e| format!("entity-{}", e.id))
                } else if idx + 1 < siblings.len() {
                    siblings.get(idx + 1).map(|e| format!("entity-{}", e.id))
                } else {
                    Some(format!("phase-{}", phase_id))
                }
            });

            (phase_id, next)
        } else {
            return;
        };

        let confirmed = gx
            .modal(crate::modals::ConfirmModal::with_message(
                "Delete this entity mapping?",
            ))
            .await;

        if !confirmed {
            return;
        }

        let repo = gx.data::<MigrationRepository>();
        match repo.delete_entity_mapping(entity_id).await {
            Ok(()) => {
                gx.toast(Toast::info("Entity mapping deleted"));
                self.load_db_data(gx).await;

                // Focus next item
                if let Some(key) = next_focus {
                    cx.focus(format!("migration-tree-node-{}", key));
                }
            }
            Err(e) => {
                log::error!("Failed to delete entity mapping: {}", e);
                gx.toast(Toast::error("Failed to delete entity mapping"));
            }
        }
    }
}
