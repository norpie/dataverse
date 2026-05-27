//! Execution state machine logic for the migration editor.

use std::collections::HashMap;
use std::collections::HashSet;

use chrono::Utc;
use rafter::prelude::*;
use uuid::Uuid;

use crate::apps::migration::execution::EntityBatches;
use crate::apps::migration::execution::EntityProgress;
use crate::apps::migration::execution::ExecutionError;
use crate::apps::migration::execution::ExecutionStatus;
use crate::apps::migration::execution::SubPhase;
use crate::apps::migration::execution::SubPhaseProgress;
use crate::apps::migration::execution::SubPhaseStatus;
use crate::apps::migration::execution::{
    generate_activate_pass, generate_associate_pass, generate_create_pass,
    generate_deactivate_pass, generate_delete_pass, generate_disassociate_pass,
    generate_update_pass, total_operations,
};
use crate::apps::migration::repository::MigrationRepository;
use crate::apps::migration::repository::NewPhaseRun;
use crate::apps::migration::types::EntityMapping;
use crate::apps::migration::types::PhaseRunStatus;
use crate::apps::queue::Queue;
use crate::apps::queue::api::AddItems;
use crate::apps::queue::api::GetItemResults;
use crate::apps::queue::api::NewItem;
use crate::apps::queue::api::QueueItemCompleted;
use crate::apps::queue::types::QueuePayload;
use crate::modals::ErrorModal;

use super::MigrationEditor;

impl MigrationEditor {
    /// Start execution after metadata and comparisons are ready.
    ///
    /// Called from the F10 wiring (Step 6) after:
    /// - exec_comparisons, exec_metadata, exec_entity_mappings are set
    /// - exec_phase_name, exec_env_id, exec_account_id are set
    /// - Already navigated to Page::Execute
    pub(super) async fn start_execution(&self, gx: &GlobalContext) {
        // Create PhaseRun in DB
        let phase_id = {
            let lua_phase_id = self.exec_phase_id.get();
            if lua_phase_id != 0 {
                lua_phase_id
            } else {
                self.exec_entity_mappings
                    .with_ref(|ems| ems.first().map(|em| em.phase_id).unwrap_or(0))
            }
        };

        let repo = gx.data::<MigrationRepository>();
        let phase_run_id = match repo
            .create_phase_run(NewPhaseRun {
                phase_id,
                started_at: Utc::now(),
            })
            .await
        {
            Ok(id) => id,
            Err(e) => {
                log::error!("Failed to create phase run: {}", e);
                gx.toast(Toast::error("Failed to start execution"));
                self.exec_status.set(ExecutionStatus::Failed);
                return;
            }
        };
        self.exec_phase_run_id.set(phase_run_id);

        // Initialize sub-phase progress for all 5 sub-phases
        let initial_progress: Vec<SubPhaseProgress> = SubPhase::ALL
            .iter()
            .map(|sp| SubPhaseProgress {
                sub_phase: *sp,
                entities: Vec::new(),
                status: SubPhaseStatus::Waiting,
            })
            .collect();
        self.exec_sub_phase_progress.set(initial_progress);

        // Set running
        self.exec_status.set(ExecutionStatus::Running);

        // Start from the first sub-phase
        self.run_sub_phase(SubPhase::Create, gx).await;
    }

    /// Generate operations for a sub-phase and submit them to the queue.
    ///
    /// If there are 0 operations (or the pass is disabled), marks as Skipped and advances.
    ///
    /// Boxed to break async recursion with `advance_to_next_sub_phase`.
    pub(super) fn run_sub_phase<'a>(
        &'a self,
        sub_phase: SubPhase,
        gx: &'a GlobalContext,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 'a>> {
        Box::pin(self.run_sub_phase_inner(sub_phase, gx))
    }

    async fn run_sub_phase_inner(&self, sub_phase: SubPhase, gx: &GlobalContext) {
        log::info!("[execution] Starting sub-phase: {:?}", sub_phase);

        self.exec_current_sub_phase.set(Some(sub_phase));

        // Update this sub-phase's status to Running
        self.update_sub_phase_status(sub_phase, SubPhaseStatus::Running);

        // Lua phase path: use pre-built batches directly
        let lua_batches = self.exec_lua_batches.with_ref(|lb| {
            lb.as_ref()
                .and_then(|batches| batches.get(&sub_phase).cloned())
        });

        if let Some(entity_batches) = lua_batches {
            log::info!(
                "[execution] Sub-phase {:?}: using pre-built Lua batches",
                sub_phase,
            );
            self.submit_sub_phase_batches(sub_phase, entity_batches, gx)
                .await;
            return;
        }

        // Declarative phase path: generate batches from comparisons

        // Check if any entity mapping has this pass enabled
        let entity_mappings = self.exec_entity_mappings.get();
        let any_enabled = entity_mappings
            .iter()
            .any(|em| is_pass_enabled(em, sub_phase));

        if !any_enabled {
            log::info!(
                "[execution] Sub-phase {:?} skipped — no entity mappings have it enabled",
                sub_phase,
            );
            self.update_sub_phase_status(sub_phase, SubPhaseStatus::Skipped);
            self.advance_to_next_sub_phase(sub_phase, gx).await;
            return;
        }

        // Generate operations
        let comparisons = self.exec_comparisons.get();
        let metadata = self.exec_metadata.get();

        let entity_batches: Vec<EntityBatches> = match sub_phase {
            SubPhase::Create => {
                let update_disabled: HashSet<String> = entity_mappings
                    .iter()
                    .filter(|em| !em.update_pass_enabled)
                    .map(|em| em.target_entity.clone())
                    .collect();
                let result = match generate_create_pass(&comparisons, &metadata, &update_disabled) {
                    Ok(result) => result,
                    Err(e) => {
                        log::error!("[execution] Create pass failed: {}", e);
                        gx.modal(ErrorModal::with_message("Create pass failed", &e))
                            .await;
                        self.update_sub_phase_status(sub_phase, SubPhaseStatus::Complete);
                        self.exec_status.set(ExecutionStatus::Failed);
                        return;
                    }
                };
                // Store pending lookups for the Update pass
                self.exec_pending_lookups.set(result.pending_lookups);
                result.entity_batches
            }
            SubPhase::Activate => generate_activate_pass(&comparisons, &metadata),
            SubPhase::Update => {
                let pending = self.exec_pending_lookups.get();
                let captured = self.exec_captured_ids.get();
                match generate_update_pass(&comparisons, &metadata, &pending, &captured) {
                    Ok(batches) => batches,
                    Err(e) => {
                        log::error!("[execution] Update pass failed: {}", e);
                        gx.modal(ErrorModal::with_message("Update pass failed", &e))
                            .await;
                        self.update_sub_phase_status(sub_phase, SubPhaseStatus::Complete);
                        self.exec_status.set(ExecutionStatus::Failed);
                        return;
                    }
                }
            }
            SubPhase::Associate => generate_associate_pass(&comparisons, &metadata),
            SubPhase::Disassociate => generate_disassociate_pass(&comparisons, &metadata),
            SubPhase::Deactivate => {
                let pending = self.exec_pending_lookups.get();
                let captured = self.exec_captured_ids.get();
                generate_deactivate_pass(&comparisons, &metadata, &pending, &captured)
            }
            SubPhase::Delete => generate_delete_pass(&comparisons, &metadata),
        };

        // Filter out entity batches where the pass is disabled for that entity
        let entity_batches: Vec<EntityBatches> = entity_batches
            .into_iter()
            .filter(|eb| {
                entity_mappings
                    .iter()
                    .any(|em| em.target_entity == eb.entity && is_pass_enabled(em, sub_phase))
            })
            .collect();

        self.submit_sub_phase_batches(sub_phase, entity_batches, gx)
            .await;
    }

    /// Submit pre-built entity batches to the queue for a sub-phase.
    ///
    /// Shared by both the declarative path (after `generate_*_pass`) and the
    /// Lua path (pre-built batches from `exec_lua_batches`).
    async fn submit_sub_phase_batches(
        &self,
        sub_phase: SubPhase,
        entity_batches: Vec<EntityBatches>,
        gx: &GlobalContext,
    ) {
        let total_ops = total_operations(&entity_batches);

        if total_ops == 0 {
            log::info!(
                "[execution] Sub-phase {:?} skipped — 0 operations",
                sub_phase,
            );
            self.update_sub_phase_status(sub_phase, SubPhaseStatus::Skipped);
            self.advance_to_next_sub_phase(sub_phase, gx).await;
            return;
        }

        log::info!(
            "[execution] Sub-phase {:?}: {} operations across {} entities",
            sub_phase,
            total_ops,
            entity_batches.len(),
        );

        // Build entity progress entries
        let entity_progress: Vec<EntityProgress> = entity_batches
            .iter()
            .map(|eb| EntityProgress {
                entity: eb.entity.clone(),
                total: eb.operation_count,
                completed: 0,
                failed: 0,
            })
            .collect();

        // Update the progress for this sub-phase
        self.exec_sub_phase_progress.update(|progress| {
            if let Some(sp) = progress.iter_mut().find(|p| p.sub_phase == sub_phase) {
                sp.entities = entity_progress;
                sp.status = SubPhaseStatus::Running;
            }
        });

        // Submit batches to queue
        let env_id = self.exec_env_id.get();
        let account_id = self.exec_account_id.get();
        let priority = sub_phase_priority(sub_phase);

        let mut new_items = Vec::new();
        let mut item_entity_map: HashMap<i64, String> = self.exec_item_entity_map.get();
        let mut item_op_counts: HashMap<i64, usize> = self.exec_item_op_counts.get();
        let mut tracked_ids = Vec::new();

        for eb in entity_batches {
            for batch in eb.batches {
                let op_count = batch.operation_count();
                new_items.push((
                    NewItem {
                        priority,
                        payload: QueuePayload::Batch(batch),
                        env_id,
                        account_id,
                        source: "migration".to_string(),
                        description: format!("{} {} ({})", sub_phase.label(), eb.entity, op_count),
                    },
                    eb.entity.clone(),
                    op_count,
                ));
            }
        }

        // Submit all items at once
        let items_to_submit: Vec<NewItem> = new_items
            .iter()
            .map(|(item, _, _)| NewItem {
                priority: item.priority,
                payload: item.payload.clone(),
                env_id: item.env_id,
                account_id: item.account_id,
                source: item.source.clone(),
                description: item.description.clone(),
            })
            .collect();

        let response = match gx
            .request::<Queue, AddItems>(AddItems {
                items: items_to_submit,
            })
            .await
        {
            Ok(resp) => resp,
            Err(e) => {
                log::error!("[execution] Failed to submit batches to queue: {:?}", e);
                gx.toast(Toast::error("Failed to submit operations to queue"));
                self.exec_status.set(ExecutionStatus::Failed);
                self.finalize_execution(PhaseRunStatus::Failed, Some(e.to_string()), gx)
                    .await;
                return;
            }
        };

        // Map returned IDs to entities and op counts
        for (idx, item_id) in response.ids.iter().enumerate() {
            if let Some((_, entity, op_count)) = new_items.get(idx) {
                item_entity_map.insert(*item_id, entity.clone());
                item_op_counts.insert(*item_id, *op_count);
                tracked_ids.push(*item_id);
            }
        }

        // Update state
        self.exec_item_entity_map.set(item_entity_map);
        self.exec_item_op_counts.set(item_op_counts);
        self.exec_tracked_item_ids.set(tracked_ids.clone());
        self.exec_all_item_ids.update(|all| {
            all.extend(tracked_ids);
        });

        log::info!(
            "[execution] Sub-phase {:?}: submitted {} queue items",
            sub_phase,
            response.ids.len(),
        );
    }

    /// Handle a queue item completion event.
    ///
    /// Called from the `#[event_handler]` in mod.rs.
    pub(super) async fn handle_item_completed(
        &self,
        event: &QueueItemCompleted,
        gx: &GlobalContext,
    ) {
        let item_id = event.item_id;

        // Check if this item belongs to our current execution
        let is_tracked = self
            .exec_tracked_item_ids
            .with_ref(|ids| ids.contains(&item_id));
        if !is_tracked {
            return;
        }

        // Not running? Ignore stale events.
        if self.exec_status.get() != ExecutionStatus::Running {
            return;
        }

        let current_sub_phase = match self.exec_current_sub_phase.get() {
            Some(sp) => sp,
            None => return,
        };

        let entity = self
            .exec_item_entity_map
            .with_ref(|m| m.get(&item_id).cloned());
        let op_count = self
            .exec_item_op_counts
            .with_ref(|m| m.get(&item_id).copied().unwrap_or(0));

        log::debug!(
            "[execution] Item {} completed: status={:?}, entity={:?}, ops={}",
            item_id,
            event.status,
            entity,
            op_count,
        );

        // Fetch results to capture created IDs and errors
        let results_response = gx
            .request::<Queue, GetItemResults>(GetItemResults { item_id })
            .await;

        let mut created_ids = Vec::new();
        let mut errors = Vec::new();
        let mut success_count = 0usize;
        let mut failure_count = 0usize;

        if let Ok(response) = results_response {
            for exec_with_results in &response.executions {
                for op_result in &exec_with_results.results {
                    if op_result.success {
                        success_count += 1;
                        // Capture created IDs from Create operations
                        if op_result.operation_type.as_deref() == Some("create") {
                            if let (Some(content_id), Some(result_data)) =
                                (&op_result.content_id, &op_result.result_data)
                            {
                                if let Some(id) = parse_created_id(result_data) {
                                    created_ids.push((content_id.clone(), id));
                                }
                            }
                        }
                    } else {
                        failure_count += 1;
                        let error_msg = op_result
                            .error_message
                            .clone()
                            .unwrap_or_else(|| "Unknown error".to_string());

                        errors.push(ExecutionError {
                            sub_phase: current_sub_phase,
                            entity: entity.clone().unwrap_or_default(),
                            record_id: op_result.content_id.clone(),
                            message: error_msg,
                        });
                    }
                }
            }
        } else {
            // If we can't fetch results, treat the whole batch as failed
            log::warn!(
                "[execution] Could not fetch results for item {} — counting all {} ops as failed",
                item_id,
                op_count,
            );
            failure_count = op_count;
            if let Some(error) = &event.error {
                errors.push(ExecutionError {
                    sub_phase: current_sub_phase,
                    entity: entity.clone().unwrap_or_default(),
                    record_id: None,
                    message: error.clone(),
                });
            }
        }

        // Store captured IDs
        if !created_ids.is_empty() {
            self.exec_captured_ids.update(|captured| {
                for (content_id, target_id) in created_ids {
                    captured.insert(content_id, target_id);
                }
            });
        }

        // Store errors
        if !errors.is_empty() {
            self.exec_errors.update(|errs| {
                errs.extend(errors);
            });
        }

        // Update entity progress
        if let Some(entity_name) = &entity {
            self.exec_sub_phase_progress.update(|progress| {
                if let Some(sp) = progress
                    .iter_mut()
                    .find(|p| p.sub_phase == current_sub_phase)
                {
                    if let Some(ep) = sp.entities.iter_mut().find(|e| &e.entity == entity_name) {
                        ep.completed += success_count + failure_count;
                        ep.failed += failure_count;
                    }
                }
            });
        }

        // Remove from tracked items
        self.exec_tracked_item_ids.update(|ids| {
            ids.retain(|id| *id != item_id);
        });

        // Check if all items for this sub-phase are done
        let remaining = self.exec_tracked_item_ids.with_ref(|ids| ids.len());

        if remaining == 0 {
            log::info!("[execution] Sub-phase {:?} complete", current_sub_phase,);
            self.update_sub_phase_status(current_sub_phase, SubPhaseStatus::Complete);
            self.advance_to_next_sub_phase(current_sub_phase, gx).await;
        }
    }

    /// Advance to the next sub-phase after the current one completes.
    async fn advance_to_next_sub_phase(&self, completed: SubPhase, gx: &GlobalContext) {
        let next = next_sub_phase(completed);

        match next {
            Some(next_sp) => {
                log::info!(
                    "[execution] Advancing from {:?} to {:?}",
                    completed,
                    next_sp,
                );
                self.run_sub_phase(next_sp, gx).await;
            }
            None => {
                // All sub-phases done
                log::info!("[execution] All sub-phases complete");
                let has_errors = self.exec_errors.with_ref(|e| !e.is_empty());
                let status = if has_errors {
                    ExecutionStatus::Failed
                } else {
                    ExecutionStatus::Complete
                };
                self.exec_status.set(status);

                let phase_run_status = if has_errors {
                    PhaseRunStatus::Failed
                } else {
                    PhaseRunStatus::Completed
                };

                let error_summary = if has_errors {
                    let count = self.exec_errors.with_ref(|e| e.len());
                    Some(format!("{} operation(s) failed", count))
                } else {
                    None
                };

                self.finalize_execution(phase_run_status, error_summary, gx)
                    .await;
            }
        }
    }

    /// Finalize execution by updating the PhaseRun in the DB.
    pub(super) async fn finalize_execution(
        &self,
        status: PhaseRunStatus,
        error: Option<String>,
        gx: &GlobalContext,
    ) {
        let phase_run_id = self.exec_phase_run_id.get();
        let all_item_ids = self.exec_all_item_ids.get();

        let repo = gx.data::<MigrationRepository>();
        if let Err(e) = repo
            .update_phase_run_status(phase_run_id, status, Some(all_item_ids), error)
            .await
        {
            log::error!("[execution] Failed to update phase run status: {}", e);
        }
    }

    /// Update a sub-phase's status in the progress list.
    fn update_sub_phase_status(&self, sub_phase: SubPhase, status: SubPhaseStatus) {
        self.exec_sub_phase_progress.update(|progress| {
            if let Some(sp) = progress.iter_mut().find(|p| p.sub_phase == sub_phase) {
                sp.status = status;
            }
        });
    }
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Check if a sub-phase pass is enabled for the given entity mapping.
fn is_pass_enabled(em: &EntityMapping, sub_phase: SubPhase) -> bool {
    match sub_phase {
        SubPhase::Create => em.create_pass_enabled,
        SubPhase::Activate => em.activate_pass_enabled,
        SubPhase::Update => em.update_pass_enabled,
        SubPhase::Associate => em.associate_pass_enabled,
        SubPhase::Disassociate => em.disassociate_pass_enabled,
        SubPhase::Deactivate => em.deactivate_pass_enabled,
        SubPhase::Delete => em.delete_pass_enabled,
    }
}

/// Get the queue priority for a sub-phase (higher = more urgent).
fn sub_phase_priority(sub_phase: SubPhase) -> i32 {
    match sub_phase {
        SubPhase::Create => 70,
        SubPhase::Activate => 60,
        SubPhase::Update => 50,
        SubPhase::Associate => 40,
        SubPhase::Disassociate => 30,
        SubPhase::Deactivate => 20,
        SubPhase::Delete => 10,
    }
}

/// Get the next sub-phase after the given one, or None if it was the last.
fn next_sub_phase(current: SubPhase) -> Option<SubPhase> {
    let all = SubPhase::ALL;
    let idx = all.iter().position(|sp| *sp == current)?;
    all.get(idx + 1).copied()
}

/// Parse a created record ID from result_data JSON like `{"id":"<uuid>"}`.
fn parse_created_id(result_data: &str) -> Option<Uuid> {
    // Simple JSON parsing — result_data is always `{"id":"<uuid>"}`
    let id_str = result_data
        .strip_prefix(r#"{"id":""#)?
        .strip_suffix(r#""}"#)?;
    Uuid::parse_str(id_str).ok()
}
