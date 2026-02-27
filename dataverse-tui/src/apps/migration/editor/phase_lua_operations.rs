//! Phase-level Lua execution entry point.
//!
//! When a Lua phase is selected for execution, this module handles the full flow:
//! warning → fetch data → run script → resolve metadata → build batches → confirm → execute.

use std::collections::HashMap;

use rafter::prelude::*;

use crate::apps::migration::comparison::matching::parse_lua_declare;
use crate::apps::migration::comparison::phase_lua::execute_phase_lua;
use crate::apps::migration::execution::phase_lua::build_phase_lua_batches;
use crate::apps::migration::execution::phase_lua::collect_referenced_entities;
use crate::apps::migration::execution::phase_lua::collect_referenced_relationships;
use crate::apps::migration::execution::phase_lua::ResolvedRelationship;
use crate::apps::migration::types::Phase;
use crate::modals::odata_fetch::ODataFetchModal;
use crate::modals::odata_fetch::ODataFetchTask;
use crate::modals::ConfirmModal;
use crate::modals::ErrorAcknowledgmentModal;
use crate::modals::LoadingModal;
use crate::systems::client_management::ClientManagement;
use crate::systems::client_management::GetAnyClient;
use dataverse_lib::api::query::odata::QueryBuilder;
use dataverse_lib::model::metadata::ExecutionMetadata;
use dataverse_lib::model::Entity;

use super::MigrationEditor;
use super::Page;

impl MigrationEditor {
    /// Execute a phase-level Lua phase directly (no preview).
    ///
    /// Flow: warning → fetch data → run script → resolve metadata → build batches
    /// → confirm → execute.
    pub(super) async fn run_lua_phase(&self, phase: Phase, gx: &GlobalContext) {
        // 1. Warning modal
        let confirmed = gx
            .modal(ConfirmModal::with_message(format!(
                "Lua phases execute directly without preview.\n\
                 Continue with phase \"{}\"?",
                phase.name,
            )))
            .await;
        if !confirmed {
            return;
        }

        // 2. Validate script
        let script = match &phase.lua_script {
            Some(s) if !s.trim().is_empty() => s.clone(),
            _ => {
                gx.toast(Toast::warning("Phase has no Lua script"));
                return;
            }
        };

        // 3. Parse M.declare() to find what data to fetch
        let declare = match parse_lua_declare(&script) {
            Ok(d) => d,
            Err(e) => {
                gx.modal(ErrorAcknowledgmentModal::new(
                    "Lua Script Error".into(),
                    format!("Failed to parse M.declare(): {e}"),
                ))
                .await;
                return;
            }
        };

        // 4. Build fetch tasks from declared entities
        let source_client = self.source_client.get().clone();
        let target_client = self.target_client.get().clone();

        let mut tasks = Vec::new();
        let mut task_keys: Vec<(bool, String)> = Vec::new(); // (is_source, entity_name)

        for (entity, fields) in &declare.source_entities {
            let select: Vec<&str> = fields.iter().map(|s| s.as_str()).collect();
            let query = QueryBuilder::new(Entity::logical(entity)).select(&select);
            tasks.push(ODataFetchTask::new(
                format!("Source: {entity}"),
                source_client.clone(),
                query,
            ));
            task_keys.push((true, entity.clone()));
        }

        for (entity, fields) in &declare.target_entities {
            let select: Vec<&str> = fields.iter().map(|s| s.as_str()).collect();
            let query = QueryBuilder::new(Entity::logical(entity)).select(&select);
            tasks.push(ODataFetchTask::new(
                format!("Target: {entity}"),
                target_client.clone(),
                query,
            ));
            task_keys.push((false, entity.clone()));
        }

        if tasks.is_empty() {
            gx.toast(Toast::warning(
                "M.declare() returned no entities to fetch",
            ));
            return;
        }

        // 5. Fetch data
        let fetch_results = match gx.modal(ODataFetchModal::create(tasks)).await {
            Ok(results) => results,
            Err(e) => {
                gx.modal(ErrorAcknowledgmentModal::new(
                    "Fetch Failed".into(),
                    format!("Data fetch failed: {e}"),
                ))
                .await;
                return;
            }
        };

        // Split results into source_data / target_data
        let mut source_data: HashMap<String, Vec<dataverse_lib::model::Record>> = HashMap::new();
        let mut target_data: HashMap<String, Vec<dataverse_lib::model::Record>> = HashMap::new();

        for (idx, records) in fetch_results.into_iter().enumerate() {
            let (is_source, entity_name) = &task_keys[idx];
            if *is_source {
                source_data.insert(entity_name.clone(), records);
            } else {
                target_data.insert(entity_name.clone(), records);
            }
        }

        // 6. Run M.resolve()
        let operations = match execute_phase_lua(&script, &source_data, &target_data) {
            Ok(ops) => ops,
            Err(e) => {
                gx.modal(ErrorAcknowledgmentModal::new(
                    "Lua Script Error".into(),
                    format!("M.resolve() failed:\n{e}"),
                ))
                .await;
                return;
            }
        };

        if operations.is_empty() {
            gx.toast(Toast::info("Lua script returned 0 operations"));
            return;
        }

        // 7. Collect referenced entities for metadata resolution
        let referenced_entities = collect_referenced_entities(&operations);
        let referenced_relationships = collect_referenced_relationships(&operations);

        // 8. Resolve metadata + relationships via LoadingModal
        let tc = target_client.clone();
        let ref_ents = referenced_entities.clone();
        let ref_rels = referenced_relationships.clone();

        let metadata_result: Result<
            (
                HashMap<String, ExecutionMetadata>,
                HashMap<String, ResolvedRelationship>,
            ),
            String,
        > = gx
            .modal(LoadingModal::run_with_default_updates(
                "Resolving metadata...",
                || Err("Cancelled".to_string()),
                |updater| {
                    let tc = tc.clone();
                    let ref_ents = ref_ents.clone();
                    let ref_rels = ref_rels.clone();
                    async move {
                        let mut metadata: HashMap<String, ExecutionMetadata> = HashMap::new();

                        // Resolve execution metadata for each referenced entity
                        for entity_name in &ref_ents {
                            updater.update(format!("Resolving metadata: {entity_name}"));
                            match tc.metadata().entity(entity_name.as_str()).await {
                                Ok(em) => {
                                    let exec_meta = em.execution_metadata().map_err(|e| {
                                        format!("Metadata error for {entity_name}: {e}")
                                    })?;
                                    metadata.insert(entity_name.clone(), exec_meta);
                                }
                                Err(e) => {
                                    return Err(format!(
                                        "Failed to fetch metadata for {entity_name}: {e}"
                                    ));
                                }
                            }
                        }

                        // Resolve lookup target entities not already in metadata
                        let lookup_targets: Vec<String> = metadata
                            .values()
                            .flat_map(|m| m.lookup_targets.values().flatten())
                            .filter(|name| !metadata.contains_key(*name))
                            .cloned()
                            .collect::<std::collections::HashSet<_>>()
                            .into_iter()
                            .collect();

                        for entity_name in &lookup_targets {
                            updater
                                .update(format!("Resolving lookup target: {entity_name}"));
                            match tc.metadata().entity(entity_name.as_str()).await {
                                Ok(em) => match em.execution_metadata() {
                                    Ok(exec_meta) => {
                                        metadata.insert(entity_name.clone(), exec_meta);
                                    }
                                    Err(e) => {
                                        log::warn!(
                                            "Metadata error for lookup target {entity_name}: \
                                             {e} — lookups to this entity may fail",
                                        );
                                    }
                                },
                                Err(e) => {
                                    log::warn!(
                                        "Failed to fetch metadata for lookup target \
                                         {entity_name}: {e} — lookups to this entity may fail",
                                    );
                                }
                            }
                        }

                        // Step 10: Resolve N:N relationship metadata
                        let mut relationships: HashMap<String, ResolvedRelationship> =
                            HashMap::new();

                        for (entity1, rel_schema_name) in &ref_rels {
                            updater.update(format!(
                                "Resolving relationship: {rel_schema_name}"
                            ));

                            let entity_meta =
                                tc.metadata().entity(entity1.as_str()).await.map_err(|e| {
                                    format!(
                                        "Failed to fetch entity metadata for '{entity1}': {e}"
                                    )
                                })?;

                            let m2m = entity_meta
                                .many_to_many_relationships
                                .iter()
                                .find(|r| r.schema_name == *rel_schema_name)
                                .ok_or_else(|| {
                                    format!(
                                        "Relationship '{rel_schema_name}' not found on \
                                         entity '{entity1}'"
                                    )
                                })?;

                            let nav_property = m2m
                                .navigation_property_for(entity1)
                                .ok_or_else(|| {
                                    format!(
                                        "No navigation property for '{rel_schema_name}' on \
                                         '{entity1}'"
                                    )
                                })?
                                .to_string();

                            let other_entity =
                                m2m.other_entity(entity1).ok_or_else(|| {
                                    format!(
                                        "Cannot determine other entity for '{rel_schema_name}' \
                                         from '{entity1}'"
                                    )
                                })?;

                            // Ensure the other entity's metadata is resolved
                            if !metadata.contains_key(other_entity) {
                                updater.update(format!(
                                    "Resolving metadata: {other_entity}"
                                ));
                                match tc.metadata().entity(other_entity).await {
                                    Ok(em) => {
                                        let exec_meta =
                                            em.execution_metadata().map_err(|e| {
                                                format!(
                                                    "Metadata error for {other_entity}: {e}"
                                                )
                                            })?;
                                        metadata.insert(other_entity.to_string(), exec_meta);
                                    }
                                    Err(e) => {
                                        return Err(format!(
                                            "Failed to fetch metadata for {other_entity}: {e}"
                                        ));
                                    }
                                }
                            }

                            let entity1_set = metadata
                                .get(entity1)
                                .map(|m| m.entity_set_name.clone())
                                .ok_or_else(|| {
                                    format!("No metadata for entity '{entity1}'")
                                })?;

                            let entity2_set = metadata
                                .get(other_entity)
                                .map(|m| m.entity_set_name.clone())
                                .ok_or_else(|| {
                                    format!("No metadata for entity '{other_entity}'")
                                })?;

                            relationships.insert(
                                rel_schema_name.clone(),
                                ResolvedRelationship {
                                    entity1_set,
                                    entity2_set,
                                    nav_property,
                                },
                            );
                        }

                        Ok((metadata, relationships))
                    }
                },
            ))
            .await;

        let (metadata, relationships) = match metadata_result {
            Ok(data) => data,
            Err(e) => {
                log::error!("[lua_phase] Failed to resolve metadata: {e}");
                gx.modal(ErrorAcknowledgmentModal::new(
                    "Metadata Error".into(),
                    format!("Failed to resolve metadata:\n{e}"),
                ))
                .await;
                return;
            }
        };

        // 9. Build batches
        let result = match build_phase_lua_batches(operations, &metadata, &relationships) {
            Ok(r) => r,
            Err(e) => {
                gx.modal(ErrorAcknowledgmentModal::new(
                    "Batch Build Error".into(),
                    format!("Failed to build operation batches:\n{e}"),
                ))
                .await;
                return;
            }
        };

        // 10. Confirmation modal with operation summary
        let counts = &result.counts;
        let mut summary_parts = Vec::new();
        if counts.create > 0 {
            summary_parts.push(format!("{} creates", counts.create));
        }
        if counts.update > 0 {
            summary_parts.push(format!("{} updates", counts.update));
        }
        if counts.associate > 0 {
            summary_parts.push(format!("{} associates", counts.associate));
        }
        if counts.disassociate > 0 {
            summary_parts.push(format!("{} disassociates", counts.disassociate));
        }
        if counts.deactivate > 0 {
            summary_parts.push(format!("{} deactivates", counts.deactivate));
        }
        if counts.delete > 0 {
            summary_parts.push(format!("{} deletes", counts.delete));
        }

        let confirm_msg = format!(
            "Execute Lua phase \"{}\"?\n{}",
            phase.name,
            summary_parts.join(", "),
        );

        if !gx.modal(ConfirmModal::with_message(confirm_msg)).await {
            return;
        }

        // 11. Resolve account_id
        let env_id = self.migration.get().target_environment_id;

        let account_id = match gx
            .request_system::<ClientManagement, GetAnyClient>(GetAnyClient { env_id })
            .await
        {
            Ok(Ok(info)) => info.account_id,
            Ok(Err(e)) => {
                log::error!("[lua_phase] Failed to get target account: {e}");
                gx.toast(Toast::error("Failed to resolve target account"));
                return;
            }
            Err(e) => {
                log::error!("[lua_phase] Failed to request target client: {e:?}");
                gx.toast(Toast::error("Failed to resolve target account"));
                return;
            }
        };

        // 12. Set execution state
        self.exec_phase_name.set(phase.name.clone());
        self.exec_phase_id.set(phase.id);
        self.exec_lua_batches.set(Some(result.batches));
        self.exec_metadata.set(metadata);
        self.exec_comparisons.set(Vec::new());
        self.exec_entity_mappings.set(Vec::new());
        self.exec_env_id.set(env_id);
        self.exec_account_id.set(account_id);

        // 13. Navigate and start execution
        self.navigate(Page::Execute);
        self.start_execution(gx).await;
    }
}
