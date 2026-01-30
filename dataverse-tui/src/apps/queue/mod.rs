//! Queue app for executing Dataverse operations.

pub mod api;
mod executor;
pub mod migrations;
pub mod modals;
pub mod repository;
mod tree;
pub mod types;
mod ui;

use std::collections::{HashMap, HashSet, VecDeque};

use chrono::{DateTime, Utc};
use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{Button, Input, Select, SelectState, SelectionMode, Text, Tree, TreeState};
use tuidom::Color;

use crate::credentials::CredentialsProvider;
use crate::paths;
use crate::settings::Settings;
use crate::systems::client_management::EnvironmentAdded;
use crate::systems::client_management::EnvironmentRemoved;

use api::AddItems;
use api::AddItemsResponse;
use api::GetQueueStatus;
use api::QueueItemCompleted;
use api::QueueReady;
use api::QueueStatusChanged;
use repository::ListFilter;
use repository::NewQueueItem;
use repository::QueueRepository;
use repository::StatusCounts;
use tree::{QueueTreeNode, build_tree_nodes};
use types::ItemStatus;
use types::StatusFilter;

/// Queue app for executing Dataverse operations in priority order.
#[app(name = "Queue", singleton, on_blur = Continue, autostart, default)]
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
    /// Active status filter.
    status_filter: StatusFilter,
    /// Source filter (multi-select with forced selection).
    source_filter: SelectState<String>,
    /// Previous source filter selection (to detect what changed).
    prev_source_selection: HashSet<String>,
    /// Search text for filtering by description.
    search_text: String,
    /// Track start times for running items (item_id -> started_at).
    running_start_times: HashMap<i64, DateTime<Utc>>,
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
        if let Ok(count) = repo.mark_running_as_interrupted().await
            && count > 0 {
                gx.toast(Toast::warning(format!(
                    "{} item(s) were interrupted, please review",
                    count
                )));
            }

        // Load initial counts
        if let Ok(counts) = repo.count_by_status().await {
            self.status_counts.set(counts);
        }

        // Load settings from global data
        let settings = gx.data::<Settings>();
        let max_concurrency = settings.queue.max_concurrency.get();
        let max_failures = settings.queue.max_failures.get();
        let status_filter = settings.queue.status_filter.get();
        let saved_sources = settings.queue.source_filter.get();
        let search_text = settings.queue.search_text.get();

        // Build source filter select state
        let available_sources = repo.get_sources().await.unwrap_or_default();
        let mut options = vec![("__all__".to_string(), "All sources".to_string())];
        options.extend(
            available_sources
                .iter()
                .map(|s| (s.clone(), s.clone())),
        );
        let mut source_state = SelectState::new(options)
            .with_selection(SelectionMode::Forced);
        
        // Restore previously selected sources, or default to "All sources"
        if saved_sources.is_empty() {
            source_state.selection.selected.insert("__all__".to_string());
        } else {
            for src in &saved_sources {
                if src == "__all__" || available_sources.contains(src) {
                    source_state.selection.selected.insert(src.clone());
                }
            }
            // Ensure at least one is selected (Forced mode requirement)
            if source_state.selection.selected.is_empty() {
                source_state.selection.selected.insert("__all__".to_string());
            }
        }

        self.status_filter.set(status_filter);
        // Track initial selection for change detection
        let initial_selection: HashSet<String> = source_state.selection.selected.clone();
        self.source_filter.set(source_state);
        self.prev_source_selection.set(initial_selection);
        self.search_text.set(search_text);

        // Load initial tree (uses filter state)
        self.refresh_tree(&repo).await;

        self.repository.set(Some(repo));

        self.is_running.set(false);
        self.running_count.set(0);
        self.max_concurrency.set(max_concurrency);
        self.failure_count.set(0);
        self.max_failures.set(max_failures);

        // Publish ready event for other systems (e.g., taskbar)
        let counts = self.status_counts.get();
        gx.publish(QueueReady {
            is_running: false,
            counts,
        });
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
        bind("p", quick_pause_resume);
        bind("r", quick_retry);
        bind("d", quick_delete);
        bind(",", open_settings);
        bind("f", cycle_status_filter);
        bind("/", focus_search);
        bind("ctrl+f", toggle_source_filter);
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
                    self.running_start_times.update(|times| {
                        times.insert(item.id, Utc::now());
                    });
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
    // Item Actions
    // =========================================================================

    #[handler]
    async fn item_activated(&self, cx: &AppContext, gx: &GlobalContext) {
        let Some(item) = self.focused_item() else {
            return;
        };

        let item_id = item.id;
        let is_ready = item.status == ItemStatus::Ready;
        let is_paused = item.status == ItemStatus::Paused;
        let is_failed = matches!(
            item.status,
            ItemStatus::Failed | ItemStatus::PartiallyFailed | ItemStatus::Interrupted
        );
        let is_non_terminal = !item.status.is_terminal() && item.status != ItemStatus::Running;

        let (x, y) = if let Some(rect) = gx.focused_element_rect() {
            (rect.x, rect.y + rect.height)
        } else {
            gx.mouse_position()
        };

        let menu = self.item_menu(item_id, is_ready, is_paused, is_failed, is_non_terminal);
        cx.context_menu(menu, x, y);
    }

    #[context_menu]
    fn item_menu(
        &self,
        item_id: i64,
        is_ready: bool,
        is_paused: bool,
        is_failed: bool,
        is_non_terminal: bool,
    ) {
        context_menu! {
            if is_ready {
                option("Pause", pause_item(item_id));
            };
            if is_paused {
                option("Resume", resume_item(item_id));
            };
            if is_failed {
                option("Retry", retry_item(item_id));
                option("View Errors", view_errors(item_id));
            };
            separator();
            if is_non_terminal {
                option("Edit", edit_item(item_id));
            };
            option("View Details", view_details(item_id));
            option("Delete", delete_item(item_id));
        }
    }

    #[handler]
    async fn pause_item(&self, item_id: i64, gx: &GlobalContext) {
        let Some(repo) = self.repository.get() else {
            return;
        };
        if repo
            .update_status(item_id, ItemStatus::Paused)
            .await
            .is_ok()
        {
            self.refresh_counts_and_tree(&repo, gx).await;
        }
    }

    #[handler]
    async fn resume_item(&self, item_id: i64, gx: &GlobalContext) {
        let Some(repo) = self.repository.get() else {
            return;
        };
        if repo.update_status(item_id, ItemStatus::Ready).await.is_ok() {
            self.refresh_counts_and_tree(&repo, gx).await;
            if self.is_running.get() {
                self.try_start_next_items(gx).await;
            }
        }
    }

    #[handler]
    async fn retry_item(&self, item_id: i64, gx: &GlobalContext) {
        let Some(repo) = self.repository.get() else {
            return;
        };
        match repo.retry_item(item_id).await {
            Ok(_new_id) => {
                gx.toast(Toast::info("Item re-queued"));
                self.refresh_counts_and_tree(&repo, gx).await;
                if self.is_running.get() {
                    self.try_start_next_items(gx).await;
                }
            }
            Err(e) => {
                log::error!("Failed to retry item {}: {}", item_id, e);
                gx.toast(Toast::error("Failed to retry item"));
            }
        }
    }

    #[handler]
    async fn delete_item(&self, item_id: i64, gx: &GlobalContext) {
        let confirmed = gx
            .modal(crate::modals::ConfirmModal::with_message("Delete this queue item?"))
            .await;
        if !confirmed {
            return;
        }

        let Some(repo) = self.repository.get() else {
            return;
        };
        if repo.delete(item_id).await.is_ok() {
            gx.toast(Toast::info("Item deleted"));
            self.refresh_counts_and_tree(&repo, gx).await;
        }
    }

    #[handler]
    async fn view_errors(&self, item_id: i64, gx: &GlobalContext) {
        let Some(repo) = self.repository.get() else {
            return;
        };
        if let Ok(executions) = repo.get_executions(item_id).await
            && let Some(exec) = executions.first()
                && let Some(error) = &exec.error {
                    gx.modal(crate::apps::queue::modals::ErrorDetailsModal::with_error(
                        error.clone(),
                    ))
                    .await;
                }
    }

    #[handler]
    async fn view_details(&self, item_id: i64, gx: &GlobalContext) {
        let Some(repo) = self.repository.get() else {
            return;
        };
        if let Ok(executions) = repo.get_executions(item_id).await {
            gx.modal(crate::apps::queue::modals::ExecutionDetailsModal::with_executions(
                executions,
            ))
            .await;
        }
    }

    #[handler]
    async fn edit_item(&self, item_id: i64, gx: &GlobalContext) {
        let Some(repo) = self.repository.get() else {
            return;
        };
        let Ok(item) = repo.get(item_id).await else {
            return;
        };
        if let Some(update) = gx
            .modal(crate::apps::queue::modals::EditItemModal::for_item(&item))
            .await
            && repo.update_item(item_id, update).await.is_ok() {
                self.refresh_counts_and_tree(&repo, gx).await;
            }
    }

    // =========================================================================
    // Quick Keybinds
    // =========================================================================

    #[handler]
    async fn quick_pause_resume(&self, gx: &GlobalContext) {
        let Some(item) = self.focused_item() else {
            return;
        };
        match item.status {
            ItemStatus::Ready => {
                self.pause_item(item.id, gx).await;
            }
            ItemStatus::Paused => {
                self.resume_item(item.id, gx).await;
            }
            _ => {}
        }
    }

    #[handler]
    async fn quick_retry(&self, gx: &GlobalContext) {
        let Some(item) = self.focused_item() else {
            return;
        };
        if matches!(
            item.status,
            ItemStatus::Failed | ItemStatus::PartiallyFailed | ItemStatus::Interrupted
        ) {
            self.retry_item(item.id, gx).await;
        }
    }

    #[handler]
    async fn quick_delete(&self, gx: &GlobalContext) {
        let Some(item) = self.focused_item() else {
            return;
        };
        self.delete_item(item.id, gx).await;
    }

    #[handler]
    async fn open_settings(&self, gx: &GlobalContext) {
        let current_concurrency = self.max_concurrency.get();
        let current_max_failures = self.max_failures.get();

        if let Some((concurrency, max_failures)) = gx
            .modal(modals::SettingsModal::with_settings(
                current_concurrency,
                current_max_failures,
            ))
            .await
        {
            self.max_concurrency.set(concurrency);
            self.max_failures.set(max_failures);

            // Persist settings
            let settings = gx.data::<Settings>();
            let _ = settings.queue.max_concurrency.set(concurrency).await;
            let _ = settings.queue.max_failures.set(max_failures).await;
        }
    }

    // =========================================================================
    // Filter Handlers
    // =========================================================================

    #[handler]
    async fn cycle_status_filter(&self, gx: &GlobalContext) {
        let current = self.status_filter.get();
        let next = current.next();
        self.status_filter.set(next);

        // Persist setting
        let settings = gx.data::<Settings>();
        let _ = settings.queue.status_filter.set(next).await;

        if let Some(repo) = self.repository.get() {
            self.refresh_tree(&repo).await;
        }
    }

    #[handler]
    async fn focus_search(&self, cx: &AppContext) {
        cx.focus("queue-search");
    }

    #[handler]
    async fn toggle_source_filter(&self, cx: &AppContext) {
        self.source_filter.update(|s| {
            s.open = !s.open;
        });
        if self.source_filter.get().open {
            cx.focus("queue-source-filter");
        }
    }

    #[handler]
    async fn search_changed(&self, gx: &GlobalContext) {
        let text = self.search_text.get();
        
        // Persist setting
        let settings = gx.data::<Settings>();
        let _ = settings.queue.search_text.set(text).await;
        
        if let Some(repo) = self.repository.get() {
            self.refresh_tree(&repo).await;
        }
    }

    #[handler]
    async fn source_filter_changed(&self, gx: &GlobalContext) {
        let current: HashSet<String> = self
            .source_filter
            .get()
            .selected_values()
            .cloned()
            .collect();
        let prev = self.prev_source_selection.get();

        // Find what was just added
        let added: Vec<_> = current.difference(&prev).cloned().collect();

        // Handle mutual exclusivity between "__all__" and specific sources
        if !added.is_empty() {
            let all_sources = "__all__".to_string();
            
            if added.contains(&all_sources) {
                // "All sources" was just selected → clear specific sources
                self.source_filter.update(|s| {
                    s.selection.selected.clear();
                    s.selection.selected.insert(all_sources.clone());
                });
            } else if current.contains(&all_sources) {
                // A specific source was selected while "All sources" was active → clear "All sources"
                self.source_filter.update(|s| {
                    s.selection.selected.remove(&all_sources);
                });
            }
        }

        // Update prev selection for next change
        let final_selection: HashSet<String> = self
            .source_filter
            .get()
            .selected_values()
            .cloned()
            .collect();
        self.prev_source_selection.set(final_selection.clone());

        // Save and refresh
        let selected: Vec<String> = final_selection.into_iter().collect();
        
        // Persist setting
        let settings = gx.data::<Settings>();
        let _ = settings.queue.source_filter.set(selected).await;
        
        if let Some(repo) = self.repository.get() {
            self.refresh_tree(&repo).await;
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

        // Refresh counts, sources, and tree
        if let Ok(counts) = repo.count_by_status().await {
            self.status_counts.set(counts.clone());
            self.publish_status_changed(gx);
        }
        self.refresh_source_options(&repo).await;
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

        // Remove from running start times
        self.running_start_times.update(|times| {
            times.remove(&event.item_id);
        });

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
        if let Some(repo) = self.repository.get()
            && let Ok(executions) = repo.get_executions(event.item_id).await
                && let Some(exec) = executions.first() {
                    self.recent_durations.update(|d| {
                        d.push_back(exec.duration_ms);
                        if d.len() > 7 {
                            d.pop_front();
                        }
                    });
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

    async fn refresh_counts_and_tree(&self, repo: &QueueRepository, gx: &GlobalContext) {
        if let Ok(counts) = repo.count_by_status().await {
            self.status_counts.set(counts);
        }
        self.refresh_source_options(repo).await;
        self.refresh_tree(repo).await;
        self.publish_status_changed(gx);
    }

    async fn refresh_tree(&self, repo: &QueueRepository) {
        let statuses = self.status_filter.get().to_statuses();

        let sources = {
            let state = self.source_filter.get();
            let selected: Vec<String> = state.selected_values().cloned().collect();
            // If "All sources" is selected or nothing is selected, don't filter by source
            if selected.is_empty() || selected.contains(&"__all__".to_string()) {
                None
            } else {
                Some(selected)
            }
        };

        let search = {
            let text = self.search_text.get();
            if text.is_empty() { None } else { Some(text) }
        };

        let filter = ListFilter {
            statuses,
            sources,
            search,
        };

        match repo.list(filter).await {
            Ok(items) => {
                // Build timing map
                let mut timing_map = std::collections::HashMap::new();

                // Add running items from memory
                let running_times = self.running_start_times.get();
                for (item_id, started_at) in running_times.iter() {
                    timing_map.insert(
                        *item_id,
                        types::ItemTiming::Running {
                            started_at: *started_at,
                        },
                    );
                }

                // Add completed items from execution history
                for item in &items {
                    if item.status.is_terminal() && !timing_map.contains_key(&item.id)
                        && let Ok(executions) = repo.get_executions(item.id).await
                            && let Some(exec) = executions.first() {
                                timing_map.insert(
                                    item.id,
                                    types::ItemTiming::Completed {
                                        duration_ms: exec.duration_ms,
                                    },
                                );
                            }
                }

                let nodes = build_tree_nodes(&items, &timing_map);
                self.tree_state.update(|s| {
                    s.set_roots(nodes);
                });
            }
            Err(e) => {
                log::error!("Failed to load queue items: {}", e);
            }
        }
    }

    async fn refresh_source_options(&self, repo: &QueueRepository) {
        let available = repo.get_sources().await.unwrap_or_default();
        self.source_filter.update(|s| {
            // Preserve current selections that still exist
            let current_selected: Vec<String> = s
                .selected_values()
                .filter(|v| *v == "__all__" || available.contains(v))
                .cloned()
                .collect();
            
            // Build options with "All sources" at the top
            let mut options = vec![("__all__".to_string(), "All sources".to_string())];
            options.extend(
                available
                    .iter()
                    .map(|src| (src.clone(), src.clone())),
            );
            s.set_options(options);
            
            // Restore valid selections
            for src in current_selected {
                s.selection.selected.insert(src);
            }
        });
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
                        self.running_start_times.update(|times| {
                            times.insert(item.id, Utc::now());
                        });

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
            (Color::var("warning"), "Running")
        } else if counts.ready > 0 || counts.blocked > 0 {
            (Color::var("primary"), "Ready")
        } else if counts.done > 0 {
            (Color::var("success"), "Done")
        } else {
            (Color::var("muted"), "Idle")
        };

        let status_filter = self.status_filter.get();
        let filter_label = status_filter.label().to_string();

        let eta_text = ui::format_eta(&self.recent_durations.get(), &counts);

        let has_completed = counts.done > 0;

        // Context-aware button logic
        let focused_item = self.focused_item();
        let show_step = !is_running
            && focused_item.as_ref().is_some_and(|item| {
                matches!(item.status, ItemStatus::Ready | ItemStatus::Paused)
            });
        let show_pause_resume = focused_item.as_ref().is_some_and(|item| {
            matches!(item.status, ItemStatus::Ready | ItemStatus::Paused)
        });
        let pause_resume_label = focused_item.as_ref().map_or("Pause", |item| {
            if item.status == ItemStatus::Paused {
                "Resume"
            } else {
                "Pause"
            }
        });
        let show_retry = focused_item.as_ref().is_some_and(|item| {
            matches!(
                item.status,
                ItemStatus::Failed | ItemStatus::PartiallyFailed | ItemStatus::Interrupted
            )
        });
        let show_delete = focused_item
            .as_ref()
            .is_some_and(|item| item.status != ItemStatus::Running);

        let preview = self.render_preview();

        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: background) {
                // Header
                row (gap: 1) {
                    text (content: "Queue") style (bold, fg: interact)
                    text (content: "●") style (fg: {status_color})
                    button (label: {status_label}, hint: "P", id: "toggle-running") on_activate: toggle_running()
                }

                // Filter and search row
                row (width: fill, gap: 2, justify: between) {
                    row (gap: 2) {
                        button (label: {filter_label}, hint: "f", id: "filter") on_activate: cycle_status_filter()
                        select (state: self.source_filter, id: "queue-source-filter", placeholder: "sources...", toggle_width: 20)
                            on_change: source_filter_changed()
                        input (state: self.search_text, id: "queue-search", placeholder: "search (/)...", width: 25)
                            on_change: search_changed()
                    }
                    row (gap: 2) {
                        row (gap: 1) {
                            text (content: {format!("{}", counts.ready)}) style (fg: primary)
                            text (content: "ready") style (fg: muted)
                        }
                        row (gap: 1) {
                            text (content: {format!("{}", counts.running)}) style (fg: warning)
                            text (content: "running") style (fg: muted)
                        }
                        row (gap: 1) {
                            text (content: {format!("{}", counts.done)}) style (fg: success)
                            text (content: "done") style (fg: muted)
                        }
                        row (gap: 1) {
                            text (content: {format!("{}", counts.failed + counts.partially_failed)}) style (fg: error)
                            text (content: "failed") style (fg: muted)
                        }
                        if !eta_text.is_empty() {
                            text (content: {eta_text}) style (fg: muted)
                        }
                        if has_completed {
                            button (label: "Clear", hint: "C", id: "clear") on_activate: clear_completed()
                        }
                    }
                }

                // Main content: 50/50 tree + preview
                row (width: fill, height: fill, gap: 1) {
                    box_ (id: "queue-tree-container", height: fill, width: fill) style (bg: surface) {
                        tree (state: self.tree_state, id: "queue-tree", width: fill, height: fill)
                            on_activate: item_activated()
                    }
                    column (width: fill, height: fill) {
                        { preview }
                    }
                }

                // Footer
                row (width: fill, justify: between) {
                    row (gap: 1) {
                        if show_step {
                            button (label: "Step", hint: "s", id: "step") on_activate: step_one()
                        }
                        if show_pause_resume {
                            button (label: {pause_resume_label}, hint: "p", id: "pause-resume") on_activate: quick_pause_resume()
                        }
                        if show_retry {
                            button (label: "Retry", hint: "r", id: "retry") on_activate: quick_retry()
                        }
                        if show_delete {
                            button (label: "Delete", hint: "d", id: "delete") on_activate: quick_delete()
                        }
                    }
                    button (label: "Settings", hint: ",", id: "settings") on_activate: open_settings()
                }
            }
        }
    }
}
