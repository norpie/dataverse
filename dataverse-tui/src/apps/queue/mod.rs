//! Queue app for executing Dataverse operations.

pub mod api;
mod executor;
pub mod migrations;
pub mod repository;
pub mod types;

use chrono::Utc;
use rafter::page;
use rafter::prelude::*;
use rafter::widgets::Text;

use crate::credentials::CredentialsProvider;
use crate::paths;
use crate::systems::client_management::EnvironmentAdded;
use crate::systems::client_management::EnvironmentRemoved;
use crate::widgets::Spinner;

use api::AddItems;
use api::AddItemsResponse;
use api::GetQueueStatus;
use api::NewItem;
use api::QueueItemCompleted;
use api::QueueStatusChanged;
use repository::NewQueueItem;
use repository::QueueRepository;
use repository::StatusCounts;
use types::ItemStatus;

/// Queue app for executing Dataverse operations in priority order.
#[app(name = "Queue", singleton, on_blur = Continue)]
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
    /// Initialization error, if any.
    init_error: Option<String>,
}

#[app_impl]
impl Queue {
    async fn on_start(&self, gx: &GlobalContext) {
        // Initialize repository
        let db_path = paths::queue_db().unwrap_or_else(|| "queue.db".into());

        match QueueRepository::new(&db_path).await {
            Ok(repo) => {
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

                self.repository.set(Some(repo));
            }
            Err(e) => {
                log::error!("Failed to initialize queue database: {}", e);
                self.init_error.set(Some(format!("Database error: {}", e)));
                gx.toast(Toast::error("Failed to initialize queue database"));
            }
        }

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

        // Refresh counts
        if let Ok(counts) = repo.count_by_status().await {
            self.status_counts.set(counts.clone());
            self.publish_status_changed(gx);
        }

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
                // Refresh counts
                if let Ok(counts) = repo.count_by_status().await {
                    self.status_counts.set(counts);
                    self.publish_status_changed(gx);
                }
                // Try to start items if running
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
                // Refresh counts
                if let Ok(counts) = repo.count_by_status().await {
                    self.status_counts.set(counts);
                    self.publish_status_changed(gx);
                }
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

        // Refresh counts
        if let Some(repo) = self.repository.get() {
            if let Ok(counts) = repo.count_by_status().await {
                self.status_counts.set(counts);
            }
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
                    // Mark as running
                    if repo
                        .update_status(item.id, ItemStatus::Running)
                        .await
                        .is_ok()
                    {
                        self.running_count.set(self.running_count.get() + 1);

                        // Spawn execution task
                        let gx = gx.clone();
                        let repo = repo.clone();
                        tokio::spawn(async move {
                            Self::execute_and_complete(item, repo, gx).await;
                        });
                    }
                }
                Ok(None) => break, // No more items
                Err(e) => {
                    log::error!("Failed to get next ready item: {}", e);
                    break;
                }
            }
        }
    }

    /// Execute an item and publish completion event.
    async fn execute_and_complete(
        item: types::QueueItem,
        repo: QueueRepository,
        gx: GlobalContext,
    ) {
        log::info!("Executing queue item {}: {}", item.id, item.description);

        // Execute the item
        let result = executor::execute_item(&item, &gx).await;

        // Update item status
        if let Err(e) = repo.update_status(item.id, result.status).await {
            log::error!("Failed to update item {} status: {}", item.id, e);
        }

        // Extract error before moving record
        let error = result.record.error.clone();

        // Save execution record
        if let Err(e) = repo.insert_execution(result.record).await {
            log::error!(
                "Failed to save execution record for item {}: {}",
                item.id,
                e
            );
        }

        // Publish completion event
        gx.publish(QueueItemCompleted {
            item_id: item.id,
            status: result.status,
            error,
        });

        log::info!(
            "Queue item {} completed with status {:?}",
            item.id,
            result.status
        );
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
        if let Some(error) = self.init_error.get() {
            return page! {
                column (padding: 2, gap: 1) style (bg: background) {
                    text (content: "Queue") style (bold, fg: interact)
                    text (content: error) style (fg: error)
                }
            };
        }

        let counts = self.status_counts.get();
        let is_running = self.is_running.get();

        let status_text = if is_running {
            format!(
                "Running ({} active, {} pending)",
                counts.running,
                counts.pending()
            )
        } else {
            format!("Paused ({} pending)", counts.pending())
        };

        let total_text = format!(
            "Total: {} | Done: {} | Failed: {}",
            counts.total(),
            counts.done,
            counts.failed + counts.partially_failed
        );

        page! {
            column (padding: 2, gap: 1, width: fill, height: fill) style (bg: background) {
                // Header
                row (gap: 2) {
                    text (content: "Queue") style (bold, fg: interact)
                    text (content: status_text) style (fg: muted)
                }

                // Placeholder for tree
                column (width: fill, height: fill, justify: center, align: center) {
                    text (content: "Queue UI coming in Phase 3") style (fg: muted)
                    spinner (id: "queue-spinner")
                }

                // Footer
                row (gap: 2) {
                    text (content: total_text) style (fg: muted)
                }
            }
        }
    }
}
