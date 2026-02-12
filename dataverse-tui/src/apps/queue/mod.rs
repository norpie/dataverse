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

use chrono::Utc;
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
use api::DeleteItems;
use api::DeleteItemsBySource;
use api::DeleteItemsResponse;
use api::GetItemResults;
use api::GetItemResultsResponse;
use api::GetQueueStatus;
use api::PauseQueue;
use api::QueueItemCompleted;
use api::QueueReady;
use api::QueueStatusChanged;
use api::ResumeQueue;
use repository::ListFilter;
use repository::NewQueueItem;
use repository::QueueRepository;
use repository::StatusCounts;
use tree::{QueueTreeNode, build_tree_nodes};
use types::ItemStatus;
use types::ItemTiming;
use types::QueueItem;
use types::StatusFilter;

/// Action button visibility and labels for queue item context.
#[derive(Clone, Debug)]
struct ActionButtonsState {
    show_step: bool,
    show_pause_resume: bool,
    pause_resume_label: &'static str,
    show_retry: bool,
    show_delete: bool,
}

/// Queue app for executing Dataverse operations in priority order.
#[app(name = "Queue", singleton, on_blur = Continue, autostart, default)]
pub struct Queue {
    /// Database repository.
    repository: Option<QueueRepository>,
    /// Whether the queue is currently executing.
    is_running: bool,
    /// All queue items (single source of truth).
    items: Vec<QueueItem>,
    /// Timing info for items (running elapsed + completed duration).
    item_timings: HashMap<i64, ItemTiming>,
    /// Maximum concurrent operations.
    max_concurrency: usize,
    /// Consecutive failure count (for auto-pause).
    failure_count: usize,
    /// Maximum failures before auto-pause.
    max_failures: usize,
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
            && count > 0
        {
            gx.toast(Toast::warning(format!(
                "{} item(s) were interrupted, please review",
                count
            )));
        }

        // Load all items into memory
        match repo.list(ListFilter::default()).await {
            Ok(all_items) => {
                self.items.set(all_items);
            }
            Err(e) => {
                log::error!("Failed to load queue items: {}", e);
            }
        }

        // Load settings from global data
        let settings = gx.data::<Settings>();
        let max_concurrency = settings.queue.max_concurrency.get();
        let max_failures = settings.queue.max_failures.get();
        let status_filter = settings.queue.status_filter.get();
        let saved_sources = settings.queue.source_filter.get();
        let search_text = settings.queue.search_text.get();

        // Build source filter select state (watch_source_options will keep it updated)
        let source_state = SelectState::new(vec![("__all__".to_string(), "All sources".to_string())])
            .with_selection(SelectionMode::Forced);

        self.status_filter.set(status_filter);
        self.source_filter.set(source_state);
        // Restore previously selected sources (will be validated by watch_source_options)
        if saved_sources.is_empty() {
            self.source_filter.update(|s| {
                s.selection.selected.insert("__all__".to_string());
            });
        } else {
            self.source_filter.update(|s| {
                for src in &saved_sources {
                    s.selection.selected.insert(src.clone());
                }
            });
        }
        let initial_selection: HashSet<String> = self
            .source_filter
            .get()
            .selection
            .selected
            .clone();
        self.prev_source_selection.set(initial_selection);
        self.search_text.set(search_text);

        self.repository.set(Some(repo));

        self.is_running.set(false);
        self.max_concurrency.set(max_concurrency);
        self.failure_count.set(0);
        self.max_failures.set(max_failures);

        // Publish ready event for other systems (e.g., taskbar)
        let counts = self.status_counts();
        gx.publish(QueueReady {
            is_running: false,
            counts,
        });
    }

    fn title(&self) -> String {
        let counts = self.status_counts();
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
    // Derived State
    // =========================================================================

    /// Compute status counts from the in-memory item list.
    #[derived]
    fn status_counts(&self) -> StatusCounts {
        let items = self.items.get();
        let mut counts = StatusCounts::default();
        for item in items.iter() {
            match item.status {
                ItemStatus::Blocked => counts.blocked += 1,
                ItemStatus::Ready => counts.ready += 1,
                ItemStatus::Paused => counts.paused += 1,
                ItemStatus::Running => counts.running += 1,
                ItemStatus::Interrupted => counts.interrupted += 1,
                ItemStatus::Done => counts.done += 1,
                ItemStatus::Failed => counts.failed += 1,
                ItemStatus::PartiallyFailed => counts.partially_failed += 1,
            }
        }
        counts
    }

    /// Compute action button visibility and labels based on focused item.
    #[derived]
    fn action_buttons(&self) -> ActionButtonsState {
        let focused = self.focused_item();
        let is_running = self.is_running.get();

        ActionButtonsState {
            show_step: !is_running
                && focused.as_ref().is_some_and(|item| {
                    matches!(item.status, ItemStatus::Ready | ItemStatus::Paused)
                }),
            show_pause_resume: focused
                .as_ref()
                .is_some_and(|item| matches!(item.status, ItemStatus::Ready | ItemStatus::Paused)),
            pause_resume_label: focused.as_ref().map_or("Pause", |item| {
                if item.status == ItemStatus::Paused {
                    "Resume"
                } else {
                    "Pause"
                }
            }),
            show_retry: focused.as_ref().is_some_and(|item| {
                matches!(
                    item.status,
                    ItemStatus::Failed | ItemStatus::PartiallyFailed | ItemStatus::Interrupted
                )
            }),
            show_delete: focused
                .as_ref()
                .is_some_and(|item| item.status != ItemStatus::Running),
        }
    }

    // =========================================================================
    // Watches
    // =========================================================================

    /// Rebuild the tree whenever items, timings, or filters change.
    #[watch]
    async fn watch_tree(&self) {
        let items = self.items.get();
        let timings = self.item_timings.get();
        let status_filter = self.status_filter.get();
        let source_filter_state = self.source_filter.get();
        let search_text = self.search_text.get();

        // Compute active filters
        let statuses = status_filter.to_statuses();
        let sources = {
            let selected: Vec<String> = source_filter_state.selected_values().cloned().collect();
            if selected.is_empty() || selected.contains(&"__all__".to_string()) {
                None
            } else {
                Some(selected)
            }
        };

        // Filter items
        let filtered: Vec<&QueueItem> = items
            .iter()
            .filter(|item| {
                if let Some(ref statuses) = statuses {
                    if !statuses.contains(&item.status) {
                        return false;
                    }
                }
                if let Some(ref sources) = sources {
                    if !sources.contains(&item.source) {
                        return false;
                    }
                }
                if !search_text.is_empty()
                    && !item
                        .description
                        .to_lowercase()
                        .contains(&search_text.to_lowercase())
                {
                    return false;
                }
                true
            })
            .collect();

        // Build and set tree nodes
        let filtered_items: Vec<_> = filtered.into_iter().cloned().collect();
        let nodes = build_tree_nodes(&filtered_items, &timings);
        self.tree_state.update(|s| {
            s.set_roots(nodes);
        });
    }

    /// Publish QueueStatusChanged whenever is_running or items change.
    #[watch]
    async fn watch_status_publish(&self, gx: &GlobalContext) {
        let is_running = self.is_running.get();
        let items = self.items.get();

        let mut counts = StatusCounts::default();
        for item in items.iter() {
            match item.status {
                ItemStatus::Blocked => counts.blocked += 1,
                ItemStatus::Ready => counts.ready += 1,
                ItemStatus::Paused => counts.paused += 1,
                ItemStatus::Running => counts.running += 1,
                ItemStatus::Interrupted => counts.interrupted += 1,
                ItemStatus::Done => counts.done += 1,
                ItemStatus::Failed => counts.failed += 1,
                ItemStatus::PartiallyFailed => counts.partially_failed += 1,
            }
        }

        gx.publish(QueueStatusChanged { is_running, counts });
    }

    /// Update source filter options whenever items change.
    #[watch]
    async fn watch_source_options(&self) {
        let items = self.items.get();

        let mut sources: Vec<String> = items
            .iter()
            .map(|i| i.source.clone())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        sources.sort();

        // Only update if options actually changed to avoid bumping generation
        let current_options: Vec<String> = self.source_filter.with_ref(|s| {
            s.options.iter().map(|(v, _)| v.clone()).collect()
        });
        let new_options: Vec<String> = std::iter::once("__all__".to_string())
            .chain(sources.iter().cloned())
            .collect();

        if current_options != new_options {
            self.source_filter.update(|s| {
                let current_selected: Vec<String> = s
                    .selected_values()
                    .filter(|v| *v == "__all__" || sources.contains(v))
                    .cloned()
                    .collect();

                let mut options = vec![("__all__".to_string(), "All sources".to_string())];
                options.extend(sources.iter().map(|src| (src.clone(), src.clone())));
                s.set_options(options);

                for src in current_selected {
                    s.selection.selected.insert(src);
                }
            });
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
    }

    #[handler]
    async fn step_one(&self, gx: &GlobalContext) {
        if self.is_running.get() {
            return;
        }

        let Some(repo) = self.repository.get() else {
            return;
        };

        // Find next ready item from in-memory list
        let next = self.items.with_ref(|items| {
            items
                .iter()
                .filter(|i| i.status == ItemStatus::Ready)
                .max_by(|a, b| {
                    a.priority
                        .cmp(&b.priority)
                        .then(b.created_at.cmp(&a.created_at))
                })
                .cloned()
        });

        match next {
            Some(item) => {
                if repo
                    .update_status(item.id, ItemStatus::Running)
                    .await
                    .is_ok()
                {
                    let item_id = item.id;
                    self.items.update(|items| {
                        if let Some(i) = items.iter_mut().find(|i| i.id == item_id) {
                            i.status = ItemStatus::Running;
                        }
                    });
                    self.item_timings.update(|timings| {
                        timings.insert(item_id, ItemTiming::Running { started_at: Utc::now() });
                    });

                    let gx = gx.clone();
                    let repo = repo.clone();
                    tokio::spawn(executor::execute_and_complete(item, repo, gx));
                }
            }
            None => {
                gx.toast(Toast::info("No ready items"));
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
                    self.items.update(|items| {
                        items.retain(|i| !i.status.is_terminal());
                    });
                    // Clean up timings for removed items
                    let remaining_ids: HashSet<i64> =
                        self.items.with_ref(|items| items.iter().map(|i| i.id).collect());
                    self.item_timings.update(|timings| {
                        timings.retain(|id, _| remaining_ids.contains(id));
                    });
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
    async fn pause_item(&self, item_id: i64) {
        let Some(repo) = self.repository.get() else {
            return;
        };
        if repo
            .update_status(item_id, ItemStatus::Paused)
            .await
            .is_ok()
        {
            self.items.update(|items| {
                if let Some(i) = items.iter_mut().find(|i| i.id == item_id) {
                    i.status = ItemStatus::Paused;
                }
            });
        }
    }

    #[handler]
    async fn resume_item(&self, item_id: i64, gx: &GlobalContext) {
        let Some(repo) = self.repository.get() else {
            return;
        };
        if repo.update_status(item_id, ItemStatus::Ready).await.is_ok() {
            self.items.update(|items| {
                if let Some(i) = items.iter_mut().find(|i| i.id == item_id) {
                    i.status = ItemStatus::Ready;
                }
            });
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
            Ok(new_id) => {
                gx.toast(Toast::info("Item re-queued"));
                // Load the new item from DB and add to memory
                if let Ok(new_item) = repo.get(new_id).await {
                    self.items.update(|items| {
                        items.push(new_item);
                    });
                }
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
            .modal(crate::modals::ConfirmModal::with_message(
                "Delete this queue item?",
            ))
            .await;
        if !confirmed {
            return;
        }

        let Some(repo) = self.repository.get() else {
            return;
        };
        if repo.delete(item_id).await.is_ok() {
            gx.toast(Toast::info("Item deleted"));
            self.items.update(|items| {
                items.retain(|i| i.id != item_id);
            });
            self.item_timings.update(|timings| {
                timings.remove(&item_id);
            });
        }
    }

    #[handler]
    async fn view_errors(&self, item_id: i64, gx: &GlobalContext) {
        let Some(repo) = self.repository.get() else {
            return;
        };
        if let Ok(executions) = repo.get_executions(item_id).await
            && let Some(exec) = executions.first()
            && let Some(error) = &exec.error
        {
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
            gx.modal(
                crate::apps::queue::modals::ExecutionDetailsModal::with_executions(executions),
            )
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
            && repo.update_item(item_id, update.clone()).await.is_ok()
        {
            self.items.update(|items| {
                if let Some(i) = items.iter_mut().find(|i| i.id == item_id) {
                    i.priority = update.priority;
                    i.description = update.description.clone();
                    i.source = update.source.clone();
                    i.env_id = update.env_id;
                }
            });
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
                self.pause_item(item.id).await;
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
                self.source_filter.update(|s| {
                    s.selection.selected.clear();
                    s.selection.selected.insert(all_sources.clone());
                });
            } else if current.contains(&all_sources) {
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

        // Persist setting
        let selected: Vec<String> = final_selection.into_iter().collect();
        let settings = gx.data::<Settings>();
        let _ = settings.queue.source_filter.set(selected).await;
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
                Ok(id) => {
                    // Load inserted item from DB to get the full record
                    if let Ok(inserted) = repo.get(id).await {
                        self.items.update(|items| {
                            items.push(inserted);
                        });
                    }
                    ids.push(id);
                }
                Err(e) => {
                    log::error!("Failed to insert queue item: {}", e);
                }
            }
        }

        // Try to start execution if running
        if self.is_running.get() {
            self.try_start_next_items(gx).await;
        }

        AddItemsResponse { ids }
    }

    #[request_handler]
    async fn handle_get_status(&self, _request: GetQueueStatus) -> StatusCounts {
        self.status_counts()
    }

    #[request_handler]
    async fn handle_get_item_results(
        &self,
        request: GetItemResults,
    ) -> GetItemResultsResponse {
        use api::ExecutionWithResults;

        let Some(repo) = self.repository.get() else {
            return GetItemResultsResponse {
                executions: vec![],
            };
        };

        let executions = match repo.get_executions(request.item_id).await {
            Ok(execs) => execs,
            Err(e) => {
                log::error!("Failed to get executions for item {}: {}", request.item_id, e);
                return GetItemResultsResponse {
                    executions: vec![],
                };
            }
        };

        let mut result = Vec::with_capacity(executions.len());
        for execution in executions {
            let results = repo
                .get_operation_results(execution.id)
                .await
                .unwrap_or_default();
            result.push(ExecutionWithResults {
                execution,
                results,
            });
        }

        GetItemResultsResponse {
            executions: result,
        }
    }

    #[request_handler]
    async fn handle_pause_queue(&self, _request: PauseQueue) {
        if self.is_running.get() {
            self.is_running.set(false);
        }
    }

    #[request_handler]
    async fn handle_resume_queue(&self, _request: ResumeQueue, gx: &GlobalContext) {
        if !self.is_running.get() {
            self.is_running.set(true);
            self.failure_count.set(0);
            self.try_start_next_items(gx).await;
        }
    }

    #[request_handler]
    async fn handle_delete_items(
        &self,
        request: DeleteItems,
    ) -> DeleteItemsResponse {
        let Some(repo) = self.repository.get() else {
            return DeleteItemsResponse { deleted: 0 };
        };

        let ids_set: HashSet<i64> = request.ids.iter().copied().collect();
        let deleted = match repo.delete_many(request.ids).await {
            Ok(count) => count,
            Err(e) => {
                log::error!("Failed to delete queue items: {}", e);
                0
            }
        };

        if deleted > 0 {
            self.items.update(|items| {
                items.retain(|i| !ids_set.contains(&i.id) || i.status == ItemStatus::Running);
            });
        }

        DeleteItemsResponse { deleted }
    }

    #[request_handler]
    async fn handle_delete_items_by_source(
        &self,
        request: DeleteItemsBySource,
    ) -> DeleteItemsResponse {
        let Some(repo) = self.repository.get() else {
            return DeleteItemsResponse { deleted: 0 };
        };

        let source = request.source.clone();
        let deleted = match repo.delete_by_source(request.source).await {
            Ok(count) => count,
            Err(e) => {
                log::error!("Failed to delete queue items by source: {}", e);
                0
            }
        };

        if deleted > 0 {
            self.items.update(|items| {
                items.retain(|i| i.source != source || i.status == ItemStatus::Running);
            });
        }

        DeleteItemsResponse { deleted }
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
                let env_id = event.id;
                self.items.update(|items| {
                    for item in items.iter_mut() {
                        if item.env_id == env_id && item.status == ItemStatus::Blocked {
                            item.status = ItemStatus::Ready;
                        }
                    }
                });
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
    async fn on_environment_removed(&self, event: EnvironmentRemoved) {
        let Some(repo) = self.repository.get() else {
            return;
        };

        // Transition Ready → Blocked for items targeting this environment
        match repo.update_environment_availability(event.id, false).await {
            Ok(count) if count > 0 => {
                log::info!("{} queue items blocked (environment removed)", count);
                let env_id = event.id;
                self.items.update(|items| {
                    for item in items.iter_mut() {
                        if item.env_id == env_id && item.status == ItemStatus::Ready {
                            item.status = ItemStatus::Blocked;
                        }
                    }
                });
            }
            Ok(_) => {}
            Err(e) => {
                log::error!("Failed to update environment availability: {}", e);
            }
        }
    }

    #[event_handler]
    async fn on_item_completed(&self, event: QueueItemCompleted, gx: &GlobalContext) {
        // Update item status in memory
        self.items.update(|items| {
            if let Some(i) = items.iter_mut().find(|i| i.id == event.item_id) {
                i.status = event.status;
            }
        });

        // Update timing: replace running timing with completed timing
        self.item_timings.update(|timings| {
            if let Some(ItemTiming::Running { started_at }) = timings.get(&event.item_id) {
                let duration_ms = (Utc::now() - *started_at).num_milliseconds();
                timings.insert(
                    event.item_id,
                    ItemTiming::Completed { duration_ms },
                );
            }
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
        if let Some(ItemTiming::Completed { duration_ms }) =
            self.item_timings.with_ref(|t| t.get(&event.item_id).copied())
        {
            self.recent_durations.update(|d| {
                d.push_back(duration_ms);
                if d.len() > 7 {
                    d.pop_front();
                }
            });
        }

        // Try to start more items
        if self.is_running.get() {
            self.try_start_next_items(gx).await;
        }
    }

    // =========================================================================
    // Internal Methods
    // =========================================================================

    async fn try_start_next_items(&self, gx: &GlobalContext) {
        let Some(repo) = self.repository.get() else {
            return;
        };

        let max = self.max_concurrency.get();
        let current = self.items.with_ref(|items| {
            items.iter().filter(|i| i.status == ItemStatus::Running).count()
        });

        // Collect ready items sorted by priority desc, created_at asc
        let mut ready: Vec<QueueItem> = self.items.with_ref(|items| {
            items
                .iter()
                .filter(|i| i.status == ItemStatus::Ready)
                .cloned()
                .collect()
        });
        ready.sort_by(|a, b| {
            b.priority
                .cmp(&a.priority)
                .then(a.created_at.cmp(&b.created_at))
        });

        let slots = max.saturating_sub(current);
        for item in ready.into_iter().take(slots) {
            if repo
                .update_status(item.id, ItemStatus::Running)
                .await
                .is_ok()
            {
                let item_id = item.id;
                self.items.update(|items| {
                    if let Some(i) = items.iter_mut().find(|i| i.id == item_id) {
                        i.status = ItemStatus::Running;
                    }
                });
                self.item_timings.update(|timings| {
                    timings.insert(item_id, ItemTiming::Running { started_at: Utc::now() });
                });

                let gx = gx.clone();
                let repo = repo.clone();
                tokio::spawn(executor::execute_and_complete(item, repo, gx));
            }
        }
    }

    // =========================================================================
    // UI
    // =========================================================================

    fn element(&self) -> Element {
        let counts = self.status_counts();
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
        let buttons = self.action_buttons();

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
                    { preview }
                }

                // Footer
                row (width: fill, justify: between) {
                    row (gap: 1) {
                        if buttons.show_step {
                            button (label: "Step", hint: "s", id: "step") on_activate: step_one()
                        }
                        if buttons.show_pause_resume {
                            button (label: {buttons.pause_resume_label}, hint: "p", id: "pause-resume") on_activate: quick_pause_resume()
                        }
                        if buttons.show_retry {
                            button (label: "Retry", hint: "r", id: "retry") on_activate: quick_retry()
                        }
                        if buttons.show_delete {
                            button (label: "Delete", hint: "d", id: "delete") on_activate: quick_delete()
                        }
                    }
                    button (label: "Settings", hint: ",", id: "settings") on_activate: open_settings()
                }
            }
        }
    }
}
