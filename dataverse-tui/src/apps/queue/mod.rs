//! Queue app for executing Dataverse operations.

pub mod api;
mod executor;
pub mod migrations;
pub mod repository;
mod tree;
pub mod types;
mod ui;

use std::collections::VecDeque;

use chrono::Utc;
use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{Text, Tree, TreeState};
use tuidom::Color;

use crate::credentials::CredentialsProvider;
use crate::paths;
use crate::systems::client_management::EnvironmentAdded;
use crate::systems::client_management::EnvironmentRemoved;

use api::AddItems;
use api::AddItemsResponse;
use api::GetQueueStatus;
use api::QueueItemCompleted;
use api::QueueStatusChanged;
use repository::ListFilter;
use repository::NewQueueItem;
use repository::QueueRepository;
use repository::StatusCounts;
use tree::{QueueTreeNode, build_tree_nodes};
use types::ItemStatus;

/// Queue app for executing Dataverse operations in priority order.
#[app(name = "Queue", singleton, on_blur = Continue, autostart)]
pub struct Queue {
    /// Database repository.
    repository: Option<QueueRepository>,
    /// Whether the queue is currently executing.
    is_running: bool,
    /// Number of currently running operations.
    running_count: usize,
    /// Maximum concurrent operations.
    max_concurrency: usize,
    /// Consecutive failure count (for auto-pause).
    failure_count: usize,
    /// Maximum failures before auto-pause.
    max_failures: usize,
    /// Current status counts.
    status_counts: StatusCounts,
    /// Tree state for the queue items.
    tree_state: TreeState<QueueTreeNode>,
    /// Last 7 execution durations for ETA calculation.
    recent_durations: VecDeque<i64>,
}

#[app_impl]
impl Queue {
    #[on_start]
    async fn on_start(&self, gx: &GlobalContext, cx: &AppContext) {
        let Some(db_path) = paths::queue_db() else {
            log::error!("Failed to resolve queue database path");
            gx.toast(Toast::error("Failed to resolve queue database path"));
            cx.close();
            return;
        };

        let repo = match QueueRepository::new(&db_path).await {
            Ok(repo) => repo,
            Err(e) => {
                log::error!("Failed to initialize queue database: {}", e);
                gx.toast(Toast::error("Failed to initialize queue database"));
                cx.close();
                return;
            }
        };

        // Check for interrupted items (crash recovery)
        if let Ok(count) = repo.mark_running_as_interrupted().await {
            if count > 0 {
                gx.toast(Toast::warning(format!(
                    "{} item(s) were interrupted, please review",
                    count
                )));
            }
        }

        // Load initial counts
        if let Ok(counts) = repo.count_by_status().await {
            self.status_counts.set(counts);
        }

        // Load initial tree
        self.refresh_tree(&repo).await;

        self.repository.set(Some(repo));

        // Set defaults
        self.is_running.set(false);
        self.running_count.set(0);
        self.max_concurrency.set(5);
        self.failure_count.set(0);
        self.max_failures.set(10);
    }

    fn title(&self) -> String {
        let counts = self.status_counts.get();
        let pending = counts.pending();
        let running = counts.running;

        if running > 0 {
            format!("Queue ({}/{})", running, pending + running)
        } else if pending > 0 {
            format!("Queue ({})", pending)
        } else {
            "Queue".to_string()
        }
    }

    // =========================================================================
    // Keybinds
    // =========================================================================

    #[keybinds]
    fn keybinds() {
        bind("P", toggle_running);
        bind("s", step_one);
        bind("C", clear_completed);
    }

    #[handler]
    async fn toggle_running(&self, gx: &GlobalContext) {
        let running = !self.is_running.get();
        self.is_running.set(running);

        if running {
            self.failure_count.set(0);
            self.try_start_next_items(gx).await;
            gx.toast(Toast::info("Queue started"));
        } else {
            gx.toast(Toast::info("Queue paused"));
        }

        self.publish_status_changed(gx);
    }

    #[handler]
    async fn step_one(&self, gx: &GlobalContext) {
        if self.is_running.get() {
            return;
        }

        let Some(repo) = self.repository.get() else {
            return;
        };

        match repo.get_next_ready().await {
            Ok(Some(item)) => {
                if repo
                    .update_status(item.id, ItemStatus::Running)
                    .await
                    .is_ok()
                {
                    self.running_count.set(self.running_count.get() + 1);
                    self.refresh_tree(&repo).await;

                    let gx = gx.clone();
                    let repo = repo.clone();
                    tokio::spawn(executor::execute_and_complete(item, repo, gx));
                }
            }
            Ok(None) => {
                gx.toast(Toast::info("No ready items"));
            }
            Err(e) => {
                log::error!("Failed to get next ready item: {}", e);
            }
        }
    }

    #[handler]
    async fn clear_completed(&self, gx: &GlobalContext) {
        let Some(repo) = self.repository.get() else {
            return;
        };

        match repo.clear_completed().await {
            Ok(count) => {
                if count > 0 {
                    gx.toast(Toast::info(format!("Cleared {} completed item(s)", count)));
                    if let Ok(counts) = repo.count_by_status().await {
                        self.status_counts.set(counts);
                    }
                    self.refresh_tree(&repo).await;
                    self.publish_status_changed(gx);
                }
            }
            Err(e) => {
                log::error!("Failed to clear completed items: {}", e);
            }
        }
    }

    // =========================================================================
    // Request Handlers
    // =========================================================================

    #[request_handler]
    async fn handle_add_items(&self, request: AddItems, gx: &GlobalContext) -> AddItemsResponse {
        let Some(repo) = self.repository.get() else {
            return AddItemsResponse { ids: vec![] };
        };

        let credentials = gx.data::<CredentialsProvider>();
        let mut ids = Vec::with_capacity(request.items.len());

        for item in request.items {
            // Check if environment exists to determine initial status
            let env_exists = credentials
                .get_environment(item.env_id)
                .await
                .ok()
                .flatten()
                .is_some();
            let status = if env_exists {
                ItemStatus::Ready
            } else {
                ItemStatus::Blocked
            };

            let new_item = NewQueueItem {
                priority: item.priority,
                status,
                payload: item.payload,
                env_id: item.env_id,
                account_id: item.account_id,
                source: item.source,
                description: item.description,
                created_at: Utc::now(),
            };

            match repo.insert(new_item).await {
                Ok(id) => ids.push(id),
                Err(e) => {
                    log::error!("Failed to insert queue item: {}", e);
                }
            }
        }

        // Refresh counts and tree
        if let Ok(counts) = repo.count_by_status().await {
            self.status_counts.set(counts.clone());
            self.publish_status_changed(gx);
        }
        self.refresh_tree(&repo).await;

        // Try to start execution if running
        if self.is_running.get() {
            self.try_start_next_items(gx).await;
        }

        AddItemsResponse { ids }
    }

    #[request_handler]
    async fn handle_get_status(&self, _request: GetQueueStatus) -> StatusCounts {
        self.status_counts.get()
    }

    // =========================================================================
    // Event Handlers
    // =========================================================================

    #[event_handler]
    async fn on_environment_added(&self, event: EnvironmentAdded, gx: &GlobalContext) {
        let Some(repo) = self.repository.get() else {
            return;
        };

        // Transition Blocked → Ready for items targeting this environment
        match repo.update_environment_availability(event.id, true).await {
            Ok(count) if count > 0 => {
                log::info!(
                    "{} queue items unblocked for environment {}",
                    count,
                    event.display_name
                );
                if let Ok(counts) = repo.count_by_status().await {
                    self.status_counts.set(counts);
                    self.publish_status_changed(gx);
                }
                self.refresh_tree(&repo).await;
                if self.is_running.get() {
                    self.try_start_next_items(gx).await;
                }
            }
            Ok(_) => {}
            Err(e) => {
                log::error!("Failed to update environment availability: {}", e);
            }
        }
    }

    #[event_handler]
    async fn on_environment_removed(&self, event: EnvironmentRemoved, gx: &GlobalContext) {
        let Some(repo) = self.repository.get() else {
            return;
        };

        // Transition Ready → Blocked for items targeting this environment
        match repo.update_environment_availability(event.id, false).await {
            Ok(count) if count > 0 => {
                log::info!("{} queue items blocked (environment removed)", count);
                if let Ok(counts) = repo.count_by_status().await {
                    self.status_counts.set(counts);
                    self.publish_status_changed(gx);
                }
                self.refresh_tree(&repo).await;
            }
            Ok(_) => {}
            Err(e) => {
                log::error!("Failed to update environment availability: {}", e);
            }
        }
    }

    #[event_handler]
    async fn on_item_completed(&self, event: QueueItemCompleted, gx: &GlobalContext) {
        // Update running count
        let running = self.running_count.get().saturating_sub(1);
        self.running_count.set(running);

        // Track failures for auto-pause
        if event.status == ItemStatus::Failed {
            let failures = self.failure_count.get() + 1;
            self.failure_count.set(failures);

            if failures >= self.max_failures.get() {
                self.is_running.set(false);
                gx.toast(Toast::warning(format!(
                    "Queue paused after {} consecutive failures",
                    failures
                )));
            }
        } else {
            // Reset failure count on success
            self.failure_count.set(0);
        }

        // Track duration for ETA
        if let Some(repo) = self.repository.get() {
            if let Ok(executions) = repo.get_executions(event.item_id).await {
                if let Some(exec) = executions.first() {
                    self.recent_durations.update(|d| {
                        d.push_back(exec.duration_ms);
                        if d.len() > 7 {
                            d.pop_front();
                        }
                    });
                }
            }
        }

        // Refresh counts and tree
        if let Some(repo) = self.repository.get() {
            if let Ok(counts) = repo.count_by_status().await {
                self.status_counts.set(counts);
            }
            self.refresh_tree(&repo).await;
        }

        self.publish_status_changed(gx);

        // Try to start more items
        if self.is_running.get() {
            self.try_start_next_items(gx).await;
        }
    }

    // =========================================================================
    // Internal Methods
    // =========================================================================

    async fn refresh_tree(&self, repo: &QueueRepository) {
        let filter = ListFilter {
            statuses: Some(vec![
                ItemStatus::Blocked,
                ItemStatus::Ready,
                ItemStatus::Paused,
                ItemStatus::Running,
                ItemStatus::Interrupted,
            ]),
            ..Default::default()
        };

        match repo.list(filter).await {
            Ok(items) => {
                let nodes = build_tree_nodes(&items);
                self.tree_state.update(|s| {
                    s.set_roots(nodes);
                });
            }
            Err(e) => {
                log::error!("Failed to load queue items: {}", e);
            }
        }
    }

    async fn try_start_next_items(&self, gx: &GlobalContext) {
        let Some(repo) = self.repository.get() else {
            return;
        };

        let max = self.max_concurrency.get();
        let current = self.running_count.get();

        // Fill available slots
        for _ in current..max {
            match repo.get_next_ready().await {
                Ok(Some(item)) => {
                    if repo
                        .update_status(item.id, ItemStatus::Running)
                        .await
                        .is_ok()
                    {
                        self.running_count.set(self.running_count.get() + 1);

                        let gx = gx.clone();
                        let repo = repo.clone();
                        tokio::spawn(executor::execute_and_complete(item, repo, gx));
                    }
                }
                Ok(None) => break,
                Err(e) => {
                    log::error!("Failed to get next ready item: {}", e);
                    break;
                }
            }
        }
    }

    fn publish_status_changed(&self, gx: &GlobalContext) {
        gx.publish(QueueStatusChanged {
            is_running: self.is_running.get(),
            counts: self.status_counts.get(),
        });
    }

    // =========================================================================
    // UI
    // =========================================================================

    fn element(&self) -> Element {
        let counts = self.status_counts.get();
        let is_running = self.is_running.get();

        let (status_color, status_label) = if !is_running {
            (Color::var("warning"), "Paused")
        } else if counts.running > 0 {
            (Color::var("success"), "Running")
        } else {
            (Color::var("primary"), "Ready")
        };
        let toggle_hint = if is_running { "pause" } else { "start" };

        let counts_text = format!(
            "{} ready  {} running  {} done  {} failed",
            counts.ready,
            counts.running,
            counts.done,
            counts.failed + counts.partially_failed,
        );
        let eta_text = ui::format_eta(&self.recent_durations.get(), &counts);

        let preview = self.render_preview();

        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: background) {
                // Header
                row (width: fill, justify: between) {
                    row (gap: 1) {
                        text (content: "Queue") style (bold, fg: interact)
                        text (content: "●") style (fg: {status_color})
                        text (content: {status_label}) style (fg: muted)
                    }
                    row (gap: 2) {
                        row (gap: 1) {
                            text (content: "P") style (fg: primary)
                            text (content: {toggle_hint}) style (fg: muted)
                        }
                        row (gap: 1) {
                            text (content: "s") style (fg: primary)
                            text (content: "step") style (fg: muted)
                        }
                        row (gap: 1) {
                            text (content: "C") style (fg: primary)
                            text (content: "clear") style (fg: muted)
                        }
                    }
                }

                // Main content: 50/50 tree + preview
                row (width: fill, height: fill, gap: 1) {
                    tree (state: self.tree_state, id: "queue-tree", width: fill, height: fill)
                    column (width: fill, height: fill) {
                        { preview }
                    }
                }

                // Footer
                row (width: fill, justify: between) {
                    text (content: {counts_text}) style (fg: muted)
                    text (content: {eta_text}) style (fg: muted)
                }
            }
        }
    }
}
