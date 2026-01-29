//! Metadata indexer system.
//!
//! Keeps metadata cache hot by periodically re-fetching before TTL expiry.
//! Runs as a background system with a dashboard modal for monitoring.

pub mod api;
mod migrations;
pub mod repository;
pub mod sync;

pub use api::*;
pub use repository::{IndexerRepository, SyncLogEntry, SyncStatus};
pub use sync::{
    execute_task, get_check_tasks, SyncTask, DEFAULT_CHECK_INTERVAL_SECS,
    DEFAULT_REFRESH_THRESHOLD_PCT,
};

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use rafter::prelude::*;
use rafter::Handler;

use crate::paths;
use crate::systems::client_management::{
    ClientManagement, EnvironmentAdded, EnvironmentRemoved, GetAuthenticatedEnvironments,
    SessionChanged,
};
use crate::systems::taskbar::StatusIndicator;

// Settings keys for persistence
const SETTING_CHECK_INTERVAL: &str = "check_interval_secs";
const SETTING_REFRESH_THRESHOLD: &str = "refresh_threshold_pct";

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

    /// Per-environment sync status for UI/events.
    env_statuses: Vec<EnvSyncStatus>,

    /// Progress tracking: tasks completed in current run.
    executed_tasks: u32,

    /// Progress tracking: total tasks in current run.
    total_tasks: u32,

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
                // Load settings from repository
                let check_interval = repo
                    .get_setting::<u64>(SETTING_CHECK_INTERVAL)
                    .await
                    .ok()
                    .flatten()
                    .unwrap_or(DEFAULT_CHECK_INTERVAL_SECS);
                let refresh_threshold = repo
                    .get_setting::<u64>(SETTING_REFRESH_THRESHOLD)
                    .await
                    .ok()
                    .flatten()
                    .unwrap_or(DEFAULT_REFRESH_THRESHOLD_PCT);

                self.check_interval_secs.set(check_interval);
                self.refresh_threshold_pct.set(refresh_threshold);
                self.repository.set(Some(repo));

                log::info!(
                    "[Indexer] Loaded settings: check_interval={}s, refresh_threshold={}%",
                    check_interval,
                    refresh_threshold
                );
            }
            Err(e) => {
                log::error!("[Indexer] Failed to open repository: {}", e);
                return;
            }
        }

        // Build initial env_statuses from authenticated environments
        self.refresh_env_statuses(gx).await;

        // Add initial check tasks for all environments
        let tasks = get_check_tasks(gx).await;
        let task_count = tasks.len() as u32;
        self.queue.update(|q| q.extend(tasks));
        self.total_tasks.set(task_count);

        log::info!(
            "[Indexer] Queued {} initial check tasks",
            task_count
        );

        // Schedule immediate processing
        let job_id = gx.schedule_after(Duration::ZERO, self.process_handler());
        self.job_id.set(Some(job_id));

        // Publish ready event
        gx.publish(IndexerReady {
            overall_status: self.compute_overall_status(),
            environments: self.env_statuses.get(),
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
        // TODO: Phase 6 - open IndexerDashboardModal
        gx.toast(Toast::info("Indexer dashboard not yet implemented"));
    }

    // =========================================================================
    // Job Processing
    // =========================================================================

    /// Create a handler closure for the scheduled job.
    fn process_handler(&self) -> Handler {
        let system = self.clone();
        Arc::new(move |hx| {
            let system = system.clone();
            let gx = hx.gx().clone();
            tokio::spawn(async move {
                system.process_queue(&gx).await;
            });
        })
    }

    /// Process the task queue - called by scheduled job.
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

            let repo = self.repository.get();
            let threshold = self.refresh_threshold_pct.get() as f64 / 100.0;

            match repo {
                Some(repo) => {
                    match execute_task(&task, &repo, gx, threshold).await {
                        Ok(follow_up_tasks) => {
                            let follow_up_count = follow_up_tasks.len() as u32;
                            if follow_up_count > 0 {
                                log::debug!(
                                    "[Indexer] Task produced {} follow-up tasks",
                                    follow_up_count
                                );
                                self.queue.update(|q| q.extend(follow_up_tasks));
                                self.total_tasks.update(|t| *t += follow_up_count);
                            }
                            self.update_status_from_task(&task, None, gx);
                        }
                        Err(e) => {
                            log::warn!("[Indexer] Task failed: {}", e);
                            self.update_status_from_task(&task, Some(e.to_string()), gx);
                        }
                    }
                }
                None => {
                    log::error!("[Indexer] No repository available");
                }
            }

            // Increment executed count
            self.executed_tasks.update(|c| *c += 1);

            // Publish status update
            self.publish_status(gx);

            // Check if we have more work
            let queue_empty = self.queue.get().is_empty();
            if queue_empty {
                // Run complete - reset progress counters
                log::info!(
                    "[Indexer] Run complete: {} tasks executed",
                    self.executed_tasks.get()
                );
                self.executed_tasks.set(0);
                self.total_tasks.set(0);
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

        let job_id = gx.schedule_after(delay, self.process_handler());
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
    async fn handle_get_status(&self, _: GetIndexerStatus, _gx: &GlobalContext) -> IndexerStatusResponse {
        IndexerStatusResponse {
            is_paused: self.is_paused.get(),
            overall_status: self.compute_overall_status(),
            environments: self.env_statuses.get(),
        }
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

        self.publish_status(gx);
    }

    #[request_handler]
    async fn handle_resume(&self, _: ResumeIndexer, gx: &GlobalContext) {
        if !self.is_paused.get() {
            return;
        }

        log::info!("[Indexer] Resuming");

        self.is_paused.set(false);

        // Schedule immediate processing
        let job_id = gx.schedule_after(Duration::ZERO, self.process_handler());
        self.job_id.set(Some(job_id));

        self.publish_status(gx);
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
                self.total_tasks.update(|t| *t += 1);
            }
        } else {
            // Trigger sync for all environments
            self.check_all_environments(gx).await;
        }

        // If not already running and not paused, schedule immediate processing
        if self.job_id.get().is_none() && !self.is_paused.get() {
            let job_id = gx.schedule_after(Duration::ZERO, self.process_handler());
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
                // Clear all environments
                for status in self.env_statuses.get() {
                    if let Err(e) = repo.clear_env_sync(status.env_id).await {
                        log::error!("[Indexer] Failed to clear env sync for {}: {}", status.env_id, e);
                    }
                }
            }
        }

        // Trigger a sync after clearing
        self.handle_trigger_sync(TriggerSync { env_id: req.env_id }, gx).await;
    }

    #[request_handler]
    async fn handle_get_sync_logs(&self, req: GetSyncLogs, _gx: &GlobalContext) -> Vec<SyncLogEntry> {
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
    async fn handle_get_settings(&self, _: GetIndexerSettings, _gx: &GlobalContext) -> SyncSettings {
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

        // Persist to repository
        if let Some(repo) = self.repository.get() {
            if let Err(e) = repo
                .set_setting(SETTING_CHECK_INTERVAL, req.check_interval_secs)
                .await
            {
                log::error!("[Indexer] Failed to persist check_interval: {}", e);
            }
            if let Err(e) = repo
                .set_setting(SETTING_REFRESH_THRESHOLD, req.refresh_threshold_pct)
                .await
            {
                log::error!("[Indexer] Failed to persist refresh_threshold: {}", e);
            }
        }

        self.publish_status(gx);
    }

    // =========================================================================
    // Event Handlers
    // =========================================================================

    #[event_handler]
    async fn on_session_changed(&self, _event: SessionChanged, gx: &GlobalContext) {
        log::debug!("[Indexer] Session changed, refreshing env statuses");
        self.refresh_env_statuses(gx).await;
        self.publish_status(gx);
    }

    #[event_handler]
    async fn on_environment_added(&self, event: EnvironmentAdded, gx: &GlobalContext) {
        log::info!("[Indexer] Environment added: {}", event.display_name);

        // Refresh env statuses to pick up the new environment
        self.refresh_env_statuses(gx).await;

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
            self.total_tasks.update(|t| *t += 1);

            // Schedule immediate processing if not paused
            if !self.is_paused.get() && self.job_id.get().is_none() {
                let job_id = gx.schedule_after(Duration::ZERO, self.process_handler());
                self.job_id.set(Some(job_id));
            }
        }

        self.publish_status(gx);
    }

    #[event_handler]
    async fn on_environment_removed(&self, event: EnvironmentRemoved, gx: &GlobalContext) {
        log::info!("[Indexer] Environment removed: {}", event.id);

        // Remove tasks for this environment from queue
        self.queue.update(|q| {
            q.retain(|task| task.env_id() != event.id);
        });

        // Remove from env_statuses
        self.env_statuses.update(|statuses| {
            statuses.retain(|s| s.env_id != event.id);
        });

        // Clear repository data for this environment
        if let Some(repo) = self.repository.get() {
            if let Err(e) = repo.clear_env_sync(event.id).await {
                log::error!("[Indexer] Failed to clear env sync for removed env: {}", e);
            }
        }

        self.publish_status(gx);
    }

    // =========================================================================
    // Helpers
    // =========================================================================

    /// Add check tasks for all authenticated environments.
    async fn check_all_environments(&self, gx: &GlobalContext) {
        let tasks = get_check_tasks(gx).await;
        let task_count = tasks.len() as u32;

        if task_count > 0 {
            self.queue.update(|q| q.extend(tasks));
            self.total_tasks.update(|t| *t += task_count);
            log::debug!("[Indexer] Added {} check tasks", task_count);
        }
    }

    /// Refresh env_statuses from authenticated environments.
    async fn refresh_env_statuses(&self, gx: &GlobalContext) {
        let environments: Vec<_> = gx
            .request_system::<ClientManagement, GetAuthenticatedEnvironments>(
                GetAuthenticatedEnvironments,
            )
            .await
            .unwrap_or_default();

        let repo = self.repository.get();

        let mut statuses = Vec::with_capacity(environments.len());
        for env in environments {
            // Try to get existing sync state from repository
            let (status, last_sync, error) = if let Some(ref repo) = repo {
                match repo.get_env_sync(env.env_id).await {
                    Ok(Some(sync)) => {
                        let indicator = match sync.status {
                            SyncStatus::Idle => StatusIndicator::Idle,
                            SyncStatus::Syncing => StatusIndicator::Running,
                            SyncStatus::Paused => StatusIndicator::Idle,
                            SyncStatus::Error => StatusIndicator::Error,
                        };
                        (indicator, sync.last_sync_at, sync.last_error)
                    }
                    _ => (StatusIndicator::Idle, None, None),
                }
            } else {
                (StatusIndicator::Idle, None, None)
            };

            statuses.push(EnvSyncStatus {
                env_id: env.env_id,
                env_name: env.environment_name,
                status,
                last_sync,
                error,
                progress: None,
            });
        }

        self.env_statuses.set(statuses);
    }

    /// Update env_statuses after a task completes.
    fn update_status_from_task(&self, task: &SyncTask, error: Option<String>, _gx: &GlobalContext) {
        let env_id = task.env_id();

        self.env_statuses.update(|statuses| {
            if let Some(status) = statuses.iter_mut().find(|s| s.env_id == env_id) {
                if let Some(err) = error {
                    status.status = StatusIndicator::PartialError;
                    status.error = Some(err);
                } else {
                    // Check if this is a completion task (FetchAllOptionSets)
                    if matches!(task, SyncTask::FetchAllOptionSets { .. }) {
                        status.status = StatusIndicator::Idle;
                        status.last_sync = Some(chrono::Utc::now());
                        status.error = None;
                        status.progress = None;
                    } else {
                        status.status = StatusIndicator::Running;
                        // Update progress based on executed/total
                        let executed = self.executed_tasks.get();
                        let total = self.total_tasks.get();
                        if total > 0 {
                            status.progress = Some((executed, total));
                        }
                    }
                }
            }
        });
    }

    /// Compute overall status from env_statuses.
    fn compute_overall_status(&self) -> StatusIndicator {
        if self.is_paused.get() {
            return StatusIndicator::Idle;
        }

        let statuses = self.env_statuses.get();

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
    fn publish_status(&self, gx: &GlobalContext) {
        gx.publish(IndexerStatusChanged {
            overall_status: self.compute_overall_status(),
            environments: self.env_statuses.get(),
        });
    }
}
