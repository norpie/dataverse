//! Metadata indexer system.
//!
//! Keeps metadata cache hot by periodically re-fetching before TTL expiry.
//! Runs as a background system with a dashboard modal for monitoring.

pub mod api;
mod migrations;
mod modal;
pub mod repository;
pub mod sync;

pub use api::*;
pub use modal::IndexerDashboardModal;
pub use repository::{IndexerRepository, SyncLogEntry, SyncStatus};
pub use sync::{
    DEFAULT_CHECK_INTERVAL_SECS, DEFAULT_REFRESH_THRESHOLD_PCT, SyncTask, execute_task,
    get_check_tasks,
};

use std::collections::{HashMap, VecDeque};
use std::time::Duration;

use rafter::prelude::*;

use crate::paths;
use crate::settings::Settings;
use crate::systems::client_management::{
    ClientManagement, EnvironmentAdded, EnvironmentRemoved, GetAuthenticatedEnvironments,
    SessionChanged,
};
use crate::systems::taskbar::StatusIndicator;

/// Background metadata indexer system.
///
/// Periodically checks cache staleness and re-fetches metadata before TTL expiry.
/// Uses an internal task queue processed by rafter scheduled jobs.
#[system]
pub struct IndexerSystem {
    /// Persistence layer.
    repository: Option<IndexerRepository>,

    /// Task queue - granular units of sync work.
    queue: VecDeque<SyncTask>,

    /// Current scheduled job ID (acts as lock - only one job at a time).
    job_id: Option<JobId>,

    /// Whether the indexer is paused.
    is_paused: bool,

    /// Per-environment progress tracking.
    env_progress: HashMap<i64, SyncProgress>,

    /// How often to check for near-expiry cache entries (seconds).
    check_interval_secs: u64,

    /// Percentage of TTL elapsed before triggering a refresh (0-100).
    refresh_threshold_pct: u64,
}

#[system_impl]
impl IndexerSystem {
    // =========================================================================
    // Lifecycle
    // =========================================================================

    #[on_start]
    async fn on_start(&self, gx: &GlobalContext) {
        log::info!("[Indexer] Starting metadata indexer system");

        // Open repository
        let Some(db_path) = paths::indexer_db() else {
            log::error!("[Indexer] Failed to get indexer database path");
            return;
        };

        match IndexerRepository::new(&db_path).await {
            Ok(repo) => {
                // Load settings from global data
                let settings = gx.data::<Settings>();
                let check_interval = settings.indexer.check_interval_secs.get();
                let refresh_threshold = settings.indexer.refresh_threshold_pct.get();
                let is_paused = settings.indexer.is_paused.get();

                self.check_interval_secs.set(check_interval);
                self.refresh_threshold_pct.set(refresh_threshold);
                self.is_paused.set(is_paused);
                self.repository.set(Some(repo));

                log::info!(
                    "[Indexer] Loaded settings: check_interval={}s, refresh_threshold={}%, paused={}",
                    check_interval,
                    refresh_threshold,
                    is_paused
                );
            }
            Err(e) => {
                log::error!("[Indexer] Failed to open repository: {}", e);
                return;
            }
        }

        // Skip initial tasks if paused
        if self.is_paused.get() {
            log::info!("[Indexer] Starting in paused state");
            let statuses = self.build_env_statuses(gx).await;
            gx.publish(IndexerReady {
                overall_status: self.compute_overall_status(&statuses),
                environments: statuses,
            });
            return;
        }

        // Add initial check tasks for all environments
        let tasks = get_check_tasks(gx).await;
        let task_count = tasks.len();
        self.queue.update(|q| q.extend(tasks));

        log::info!("[Indexer] Queued {} initial check tasks", task_count);

        // Schedule immediate processing
        let job_id = gx.schedule_after(Duration::ZERO, self.process_queue_handler());
        self.job_id.set(Some(job_id));

        // Publish ready event
        let statuses = self.build_env_statuses(gx).await;
        gx.publish(IndexerReady {
            overall_status: self.compute_overall_status(&statuses),
            environments: statuses,
        });
    }

    // =========================================================================
    // Keybinds
    // =========================================================================

    #[keybinds]
    fn keys() {
        bind("alt+i", open_dashboard);
    }

    #[handler]
    async fn open_dashboard(&self, gx: &GlobalContext) {
        let status = self.get_current_status(gx).await;
        let settings = self.get_current_settings();
        let _ = gx
            .modal(modal::IndexerDashboardModal::with_status(status, settings))
            .await;
    }

    // =========================================================================
    // Job Processing
    // =========================================================================

    /// Process the task queue - called by scheduled job.
    #[handler]
    async fn process_queue(&self, gx: &GlobalContext) {
        // Skip if paused
        if self.is_paused.get() {
            log::debug!("[Indexer] Paused, scheduling idle check");
            self.schedule_next(gx, true);
            return;
        }

        // If queue is empty, check all environments to find work
        if self.queue.get().is_empty() {
            log::debug!("[Indexer] Queue empty, checking all environments");
            self.check_all_environments(gx).await;
        }

        // Process one task if available
        let mut queue = self.queue.get();
        let task = queue.pop_front();
        self.queue.set(queue);

        if let Some(task) = task {
            log::debug!("[Indexer] Processing task: {:?}", task);

            let env_id = task.env_id();
            let repo = self.repository.get();
            let threshold = self.refresh_threshold_pct.get() as f64 / 100.0;

            match repo {
                Some(repo) => {
                    match execute_task(&task, &repo, gx, threshold).await {
                        Ok(follow_up_tasks) => {
                            let follow_up_count = follow_up_tasks.len() as u32;

                            // Update per-env progress based on task type
                            self.update_progress_on_success(&task, &follow_up_tasks);

                            if follow_up_count > 0 {
                                log::debug!(
                                    "[Indexer] Task produced {} follow-up tasks",
                                    follow_up_count
                                );
                                self.queue.update(|q| q.extend(follow_up_tasks));
                            }
                            self.persist_task_result(&task, None, follow_up_count).await;
                        }
                        Err(e) => {
                            log::warn!("[Indexer] Task failed: {}", e);
                            // Clear progress for this env on error
                            self.env_progress.update(|p| {
                                p.remove(&env_id);
                            });
                            self.persist_task_result(&task, Some(e.to_string()), 0)
                                .await;
                        }
                    }
                }
                None => {
                    log::error!("[Indexer] No repository available");
                }
            }

            // Publish status update
            self.publish_status(gx).await;

            // Check if we have more work
            let queue_empty = self.queue.get().is_empty();
            if queue_empty {
                // Run complete - clear all progress
                let total_done: u32 = self
                    .env_progress
                    .get()
                    .values()
                    .map(|p| p.entities_done + if p.optionsets_done { 1 } else { 0 })
                    .sum();
                log::info!("[Indexer] Run complete: {} tasks executed", total_done);
                self.env_progress.set(HashMap::new());
            }

            // Schedule next: immediate if more work, interval if idle
            self.schedule_next(gx, queue_empty);
        } else {
            // No work available - schedule idle check
            log::debug!("[Indexer] No tasks available, scheduling idle check");
            self.schedule_next(gx, true);
        }
    }

    /// Schedule the next processing job.
    fn schedule_next(&self, gx: &GlobalContext, idle: bool) {
        let delay = if idle {
            Duration::from_secs(self.check_interval_secs.get())
        } else {
            Duration::ZERO
        };

        let job_id = gx.schedule_after(delay, self.process_queue_handler());
        self.job_id.set(Some(job_id));

        if idle {
            log::debug!(
                "[Indexer] Scheduled idle check in {}s",
                self.check_interval_secs.get()
            );
        }
    }

    // =========================================================================
    // Request Handlers
    // =========================================================================

    #[request_handler]
    async fn handle_get_status(
        &self,
        _: GetIndexerStatus,
        gx: &GlobalContext,
    ) -> IndexerStatusResponse {
        self.get_current_status(gx).await
    }

    #[request_handler]
    async fn handle_pause(&self, _: PauseIndexer, gx: &GlobalContext) {
        if self.is_paused.get() {
            return;
        }

        log::info!("[Indexer] Pausing");

        // Cancel current job
        if let Some(job_id) = self.job_id.get() {
            gx.cancel_job(job_id);
        }
        self.job_id.set(None);
        self.is_paused.set(true);

        // Persist paused state
        let settings = gx.data::<Settings>();
        let _ = settings.indexer.is_paused.set(true).await;

        self.publish_status(gx).await;
    }

    #[request_handler]
    async fn handle_resume(&self, _: ResumeIndexer, gx: &GlobalContext) {
        if !self.is_paused.get() {
            return;
        }

        log::info!("[Indexer] Resuming");

        self.is_paused.set(false);

        // Persist paused state
        let settings = gx.data::<Settings>();
        let _ = settings.indexer.is_paused.set(false).await;

        // Schedule immediate processing
        let job_id = gx.schedule_after(Duration::ZERO, self.process_queue_handler());
        self.job_id.set(Some(job_id));

        self.publish_status(gx).await;
    }

    #[request_handler]
    async fn handle_trigger_sync(&self, req: TriggerSync, gx: &GlobalContext) {
        log::info!("[Indexer] Trigger sync requested: {:?}", req.env_id);

        if let Some(env_id) = req.env_id {
            // Get all check tasks and find the one for this environment
            let tasks = get_check_tasks(gx).await;
            let task = tasks.into_iter().find(|t| {
                if let SyncTask::CheckEnvironment { env_id: id, .. } = t {
                    *id == env_id
                } else {
                    false
                }
            });

            if let Some(task) = task {
                self.queue.update(|q| q.push_front(task));
            }
        } else {
            // Trigger sync for all environments
            self.check_all_environments(gx).await;
        }

        // Schedule immediate processing (cancel any pending idle check)
        if !self.is_paused.get() {
            if let Some(job_id) = self.job_id.get() {
                gx.cancel_job(job_id);
            }
            let job_id = gx.schedule_after(Duration::ZERO, self.process_queue_handler());
            self.job_id.set(Some(job_id));
        }
    }

    #[request_handler]
    async fn handle_clear_cache(&self, req: ClearCache, gx: &GlobalContext) {
        log::info!("[Indexer] Clear cache requested: {:?}", req.env_id);

        if let Some(repo) = self.repository.get() {
            if let Some(env_id) = req.env_id {
                // Clear specific environment
                if let Err(e) = repo.clear_env_sync(env_id).await {
                    log::error!("[Indexer] Failed to clear env sync: {}", e);
                }
            } else {
                // Clear all environments - get list from client management
                let environments: Vec<_> = gx
                    .request_system::<ClientManagement, GetAuthenticatedEnvironments>(
                        GetAuthenticatedEnvironments,
                    )
                    .await
                    .unwrap_or_default();
                for env in environments {
                    if let Err(e) = repo.clear_env_sync(env.env_id).await {
                        log::error!(
                            "[Indexer] Failed to clear env sync for {}: {}",
                            env.env_id,
                            e
                        );
                    }
                }
            }
        }

        // Trigger a sync after clearing
        self.handle_trigger_sync(TriggerSync { env_id: req.env_id }, gx)
            .await;
    }

    #[request_handler]
    async fn handle_get_sync_logs(
        &self,
        req: GetSyncLogs,
        _gx: &GlobalContext,
    ) -> Vec<SyncLogEntry> {
        let Some(repo) = self.repository.get() else {
            return vec![];
        };

        let result = if let Some(env_id) = req.env_id {
            repo.get_sync_logs(env_id, req.limit).await
        } else {
            repo.get_all_sync_logs(req.limit).await
        };

        result.unwrap_or_default()
    }

    #[request_handler]
    async fn handle_get_settings(
        &self,
        _: GetIndexerSettings,
        _gx: &GlobalContext,
    ) -> SyncSettings {
        SyncSettings {
            check_interval_secs: self.check_interval_secs.get(),
            refresh_threshold_pct: self.refresh_threshold_pct.get(),
        }
    }

    #[request_handler]
    async fn handle_update_settings(&self, req: UpdateIndexerSettings, gx: &GlobalContext) {
        log::info!(
            "[Indexer] Updating settings: check_interval={}s, refresh_threshold={}%",
            req.check_interval_secs,
            req.refresh_threshold_pct
        );

        self.check_interval_secs.set(req.check_interval_secs);
        self.refresh_threshold_pct.set(req.refresh_threshold_pct);

        // Persist to global settings
        let settings = gx.data::<Settings>();
        if let Err(e) = settings
            .indexer
            .check_interval_secs
            .set(req.check_interval_secs)
            .await
        {
            log::error!("[Indexer] Failed to persist check_interval: {}", e);
        }
        if let Err(e) = settings
            .indexer
            .refresh_threshold_pct
            .set(req.refresh_threshold_pct)
            .await
        {
            log::error!("[Indexer] Failed to persist refresh_threshold: {}", e);
        }

        self.publish_status(gx).await;
    }

    // =========================================================================
    // Event Handlers
    // =========================================================================

    #[event_handler]
    async fn on_session_changed(&self, _event: SessionChanged, gx: &GlobalContext) {
        log::debug!("[Indexer] Session changed");
        self.publish_status(gx).await;
    }

    #[event_handler]
    async fn on_environment_added(&self, event: EnvironmentAdded, gx: &GlobalContext) {
        log::info!("[Indexer] Environment added: {}", event.display_name);

        // Add a check task for the new environment
        // Note: We need account_id, but we don't have it from the event
        // The check_all_environments will pick it up on next idle check
        // For immediate response, we trigger a full check
        let tasks = get_check_tasks(gx).await;
        let new_task = tasks.into_iter().find(|t| {
            if let SyncTask::CheckEnvironment { env_id, .. } = t {
                *env_id == event.id
            } else {
                false
            }
        });

        if let Some(task) = new_task {
            self.queue.update(|q| q.push_front(task));

            // Schedule immediate processing if not paused
            if !self.is_paused.get() && self.job_id.get().is_none() {
                let job_id = gx.schedule_after(Duration::ZERO, self.process_queue_handler());
                self.job_id.set(Some(job_id));
            }
        }

        self.publish_status(gx).await;
    }

    #[event_handler]
    async fn on_environment_removed(&self, event: EnvironmentRemoved, gx: &GlobalContext) {
        log::info!("[Indexer] Environment removed: {}", event.id);

        // Remove tasks for this environment from queue
        self.queue.update(|q| {
            q.retain(|task| task.env_id() != event.id);
        });

        // Clear repository data for this environment
        if let Some(repo) = self.repository.get()
            && let Err(e) = repo.clear_env_sync(event.id).await {
                log::error!("[Indexer] Failed to clear env sync for removed env: {}", e);
            }

        self.publish_status(gx).await;
    }

    #[event_handler]
    async fn on_pause_event(&self, _event: PauseIndexerEvent, gx: &GlobalContext) {
        if self.is_paused.get() {
            return;
        }

        log::info!("[Indexer] Pausing (via event)");

        // Cancel current job
        if let Some(job_id) = self.job_id.get() {
            gx.cancel_job(job_id);
        }
        self.job_id.set(None);
        self.is_paused.set(true);

        // Persist paused state
        let settings = gx.data::<Settings>();
        let _ = settings.indexer.is_paused.set(true).await;

        self.publish_status(gx).await;
    }

    #[event_handler]
    async fn on_resume_event(&self, _event: ResumeIndexerEvent, gx: &GlobalContext) {
        if !self.is_paused.get() {
            return;
        }

        log::info!("[Indexer] Resuming (via event)");

        self.is_paused.set(false);

        // Persist paused state
        let settings = gx.data::<Settings>();
        let _ = settings.indexer.is_paused.set(false).await;

        // Schedule immediate processing
        let job_id = gx.schedule_after(Duration::ZERO, self.process_queue_handler());
        self.job_id.set(Some(job_id));

        self.publish_status(gx).await;
    }

    #[event_handler]
    async fn on_trigger_sync_event(&self, event: TriggerSyncEvent, gx: &GlobalContext) {
        log::info!("[Indexer] Trigger sync (via event): {:?}", event.env_id);

        if let Some(env_id) = event.env_id {
            // Get all check tasks and find the one for this environment
            let tasks = get_check_tasks(gx).await;
            let task = tasks.into_iter().find(|t| {
                if let SyncTask::CheckEnvironment { env_id: id, .. } = t {
                    *id == env_id
                } else {
                    false
                }
            });

            if let Some(task) = task {
                self.queue.update(|q| q.push_front(task));
            }
        } else {
            // Trigger sync for all environments
            self.check_all_environments(gx).await;
        }

        // Schedule immediate processing (cancel any pending idle check)
        if !self.is_paused.get() {
            if let Some(job_id) = self.job_id.get() {
                gx.cancel_job(job_id);
            }
            let job_id = gx.schedule_after(Duration::ZERO, self.process_queue_handler());
            self.job_id.set(Some(job_id));
        }

        self.publish_status(gx).await;
    }

    #[event_handler]
    async fn on_update_settings_event(
        &self,
        event: UpdateIndexerSettingsEvent,
        gx: &GlobalContext,
    ) {
        log::info!(
            "[Indexer] Updating settings (via event): check_interval={}s, refresh_threshold={}%",
            event.check_interval_secs,
            event.refresh_threshold_pct
        );

        self.check_interval_secs.set(event.check_interval_secs);
        self.refresh_threshold_pct.set(event.refresh_threshold_pct);

        // Persist to global settings
        let settings = gx.data::<Settings>();
        if let Err(e) = settings
            .indexer
            .check_interval_secs
            .set(event.check_interval_secs)
            .await
        {
            log::error!("[Indexer] Failed to persist check_interval: {}", e);
        }
        if let Err(e) = settings
            .indexer
            .refresh_threshold_pct
            .set(event.refresh_threshold_pct)
            .await
        {
            log::error!("[Indexer] Failed to persist refresh_threshold: {}", e);
        }

        self.publish_settings(gx);
    }

    #[event_handler]
    async fn on_open_dashboard(&self, _: OpenIndexerDashboard, gx: &GlobalContext) {
        let status = self.get_current_status(gx).await;
        let settings = SyncSettings {
            check_interval_secs: self.check_interval_secs.get(),
            refresh_threshold_pct: self.refresh_threshold_pct.get(),
        };
        let _ = gx
            .modal(IndexerDashboardModal::with_status(status, settings))
            .await;
    }

    // =========================================================================
    // Helpers
    // =========================================================================

    /// Add check tasks for all authenticated environments.
    async fn check_all_environments(&self, gx: &GlobalContext) {
        let tasks = get_check_tasks(gx).await;
        let task_count = tasks.len();

        if task_count > 0 {
            self.queue.update(|q| q.extend(tasks));
            log::debug!("[Indexer] Added {} check tasks", task_count);
        }
    }

    /// Update per-environment progress after a successful task.
    fn update_progress_on_success(&self, task: &SyncTask, follow_up_tasks: &[SyncTask]) {
        let env_id = task.env_id();

        match task {
            SyncTask::CheckEnvironment { .. } => {
                // Check tasks don't contribute to progress directly
            }
            SyncTask::FetchAllEntities { .. } => {
                // Initialize progress for this env based on follow-up tasks
                let entity_count = follow_up_tasks
                    .iter()
                    .filter(|t| matches!(t, SyncTask::FetchEntityMetadata { .. }))
                    .count() as u32;
                let has_optionsets = follow_up_tasks
                    .iter()
                    .any(|t| matches!(t, SyncTask::FetchAllOptionSets { .. }));

                self.env_progress.update(|p| {
                    p.insert(
                        env_id,
                        SyncProgress {
                            entities_total: entity_count,
                            entities_done: 0,
                            optionsets_pending: has_optionsets,
                            optionsets_done: false,
                        },
                    );
                });

                log::debug!(
                    "[Indexer] Initialized progress for env {}: {} entities, optionsets={}",
                    env_id,
                    entity_count,
                    has_optionsets
                );
            }
            SyncTask::FetchEntityMetadata { .. } => {
                // Increment entity progress
                self.env_progress.update(|p| {
                    if let Some(progress) = p.get_mut(&env_id) {
                        progress.entities_done += 1;
                    }
                });
            }
            SyncTask::FetchAllOptionSets { .. } => {
                // Mark optionsets as done
                self.env_progress.update(|p| {
                    if let Some(progress) = p.get_mut(&env_id) {
                        progress.optionsets_pending = false;
                        progress.optionsets_done = true;
                    }
                });
            }
        }
    }

    /// Build env statuses on-demand from client management + indexer DB.
    async fn build_env_statuses(&self, gx: &GlobalContext) -> Vec<EnvSyncStatus> {
        // Get environments from client management (source of truth)
        let environments: Vec<_> = gx
            .request_system::<ClientManagement, GetAuthenticatedEnvironments>(
                GetAuthenticatedEnvironments,
            )
            .await
            .unwrap_or_default();

        let repo = self.repository.get();
        let queue = self.queue.get();

        let mut statuses = Vec::with_capacity(environments.len());
        for env in environments {
            // Get sync state from indexer DB
            let (db_status, last_sync, error) = if let Some(ref repo) = repo {
                match repo.get_env_sync(env.env_id).await {
                    Ok(Some(sync)) => {
                        let indicator = match sync.status {
                            SyncStatus::Idle => StatusIndicator::Idle,
                            SyncStatus::Syncing => StatusIndicator::Running,
                            SyncStatus::Paused => StatusIndicator::Idle,
                            SyncStatus::Error => StatusIndicator::PartialError,
                        };
                        (indicator, sync.last_sync_at, sync.last_error)
                    }
                    _ => (StatusIndicator::Idle, None, None),
                }
            } else {
                (StatusIndicator::Idle, None, None)
            };

            // Only the front of queue is actively running
            let is_current = queue.front().map(|t| t.env_id()) == Some(env.env_id);
            let status = if db_status == StatusIndicator::PartialError {
                // Preserve error state
                StatusIndicator::PartialError
            } else if is_current {
                StatusIndicator::Running
            } else {
                // Not current - show Idle even if DB says Syncing
                StatusIndicator::Idle
            };

            // Get per-env progress
            let progress = self.env_progress.get().get(&env.env_id).cloned();

            statuses.push(EnvSyncStatus {
                env_id: env.env_id,
                env_name: env.environment_name,
                status,
                last_sync,
                error,
                progress,
            });
        }

        statuses
    }

    /// Persist task result to indexer DB.
    async fn persist_task_result(
        &self,
        task: &SyncTask,
        error: Option<String>,
        follow_up_count: u32,
    ) {
        let env_id = task.env_id();
        let Some(repo) = self.repository.get() else {
            return;
        };

        if let Some(err) = error {
            // Persist error status
            if let Err(e) = repo
                .upsert_env_sync(env_id, SyncStatus::Error, None, Some(err), 0, 0, 0)
                .await
            {
                log::error!("[Indexer] Failed to persist error status: {}", e);
            }
        } else {
            // Check if we should mark as complete (task succeeded)
            let is_final = matches!(task, SyncTask::FetchAllOptionSets { .. })
                || (matches!(task, SyncTask::CheckEnvironment { .. }) && follow_up_count == 0);

            if is_final {
                // Persist success - clear error
                if let Err(e) = repo
                    .upsert_env_sync(
                        env_id,
                        SyncStatus::Idle,
                        Some(chrono::Utc::now()),
                        None,
                        0,
                        0,
                        0,
                    )
                    .await
                {
                    log::error!("[Indexer] Failed to persist success status: {}", e);
                }
            }
        }
    }

    /// Compute overall status from env statuses.
    fn compute_overall_status(&self, statuses: &[EnvSyncStatus]) -> StatusIndicator {
        if self.is_paused.get() {
            return StatusIndicator::Idle;
        }

        let has_error = statuses.iter().any(|s| s.status == StatusIndicator::Error);
        let has_partial_error = statuses
            .iter()
            .any(|s| s.status == StatusIndicator::PartialError);
        let has_running = statuses
            .iter()
            .any(|s| s.status == StatusIndicator::Running);

        if has_error {
            StatusIndicator::Error
        } else if has_partial_error {
            StatusIndicator::PartialError
        } else if has_running || !self.queue.get().is_empty() {
            StatusIndicator::Running
        } else {
            StatusIndicator::Idle
        }
    }

    /// Publish status changed event.
    async fn publish_status(&self, gx: &GlobalContext) {
        log::debug!("[Indexer] Publishing IndexerStatusChanged event");
        let statuses = self.build_env_statuses(gx).await;
        gx.publish(IndexerStatusChanged {
            is_paused: self.is_paused.get(),
            overall_status: self.compute_overall_status(&statuses),
            environments: statuses,
        });
    }

    /// Publish settings changed event.
    fn publish_settings(&self, gx: &GlobalContext) {
        gx.publish(IndexerSettingsChanged {
            settings: SyncSettings {
                check_interval_secs: self.check_interval_secs.get(),
                refresh_threshold_pct: self.refresh_threshold_pct.get(),
            },
        });
    }

    /// Get current status for modal initialization.
    async fn get_current_status(&self, gx: &GlobalContext) -> IndexerStatusResponse {
        let statuses = self.build_env_statuses(gx).await;
        IndexerStatusResponse {
            is_paused: self.is_paused.get(),
            overall_status: self.compute_overall_status(&statuses),
            environments: statuses,
        }
    }

    /// Get current settings for modal initialization.
    fn get_current_settings(&self) -> SyncSettings {
        let interval = self.check_interval_secs.get();
        let threshold = self.refresh_threshold_pct.get();
        SyncSettings {
            check_interval_secs: if interval == 0 {
                DEFAULT_CHECK_INTERVAL_SECS
            } else {
                interval
            },
            refresh_threshold_pct: if threshold == 0 {
                DEFAULT_REFRESH_THRESHOLD_PCT
            } else {
                threshold
            },
        }
    }
}
