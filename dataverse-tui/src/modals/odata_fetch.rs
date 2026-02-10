//! OData fetch modal for executing multiple paginated queries in parallel with progress.
//!
//! Shows per-task progress (records fetched / total) with ETA calculation.
//!
//! # Example
//!
//! ```ignore
//! let tasks = vec![
//!     ODataFetchTask::new("Accounts", client.clone(), client.query(Entity::set("accounts")).select(&["name"])),
//!     ODataFetchTask::new("Contacts", other_client.clone(), other_client.query(Entity::set("contacts")).select(&["fullname"])),
//! ];
//!
//! let results = gx.modal(ODataFetchModal::new(tasks)).await;
//! match results {
//!     Ok(data) => { /* data: Vec<Vec<Record>> */ }
//!     Err(ODataFetchError::TaskFailed { label, error }) => { /* one task failed */ }
//!     Err(ODataFetchError::Cancelled) => { /* modal was dismissed */ }
//! }
//! ```

use std::fmt;
use std::sync::Arc;
use std::time::Instant;

use rafter::prelude::*;
use rafter::widgets::Text;
use rafter::{element, page};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use dataverse_lib::DataverseClient;
use dataverse_lib::api::query::odata::QueryBuilder;
use dataverse_lib::model::Record;

use crate::widgets::BrailleSpinner;

/// A single OData fetch task with its own client and query.
#[derive(Clone)]
pub struct ODataFetchTask {
    /// Display label for this task.
    pub label: String,
    /// Client to use for this task (allows cross-environment fetches).
    pub client: DataverseClient,
    /// The query to execute (will be paginated automatically).
    pub query: QueryBuilder,
}

impl ODataFetchTask {
    /// Create a new fetch task.
    pub fn new(label: impl Into<String>, client: DataverseClient, query: QueryBuilder) -> Self {
        Self {
            label: label.into(),
            client,
            query,
        }
    }
}

/// Error returned by `ODataFetchModal`.
#[derive(Debug, Clone)]
pub enum ODataFetchError {
    /// A task failed, and all other tasks were cancelled.
    TaskFailed {
        /// The label of the task that failed.
        label: String,
        /// The error message.
        error: String,
    },
    /// The modal was dismissed (e.g. app shutdown).
    Cancelled,
}

impl fmt::Display for ODataFetchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TaskFailed { label, error } => {
                write!(f, "Task \"{}\" failed: {}", label, error)
            }
            Self::Cancelled => write!(f, "Fetch cancelled"),
        }
    }
}

impl std::error::Error for ODataFetchError {}

/// Status of an individual fetch task.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum FetchTaskStatus {
    /// Task is waiting to start.
    #[default]
    Pending,
    /// Running the count query to determine total records.
    Counting,
    /// Fetching pages of records.
    Fetching,
    /// Task completed successfully.
    Completed,
    /// Task failed.
    Failed,
    /// Task was cancelled because another task failed.
    Cancelled,
}

/// Display info for a single fetch task (reactive state).
#[derive(Clone, Debug, Default)]
pub struct FetchTaskInfo {
    /// Display label.
    pub label: String,
    /// Current status.
    pub status: FetchTaskStatus,
    /// Total records (from count query).
    pub total_count: Option<usize>,
    /// Records fetched so far.
    pub records_fetched: usize,
}

/// Message sent from spawned tasks back to the modal.
enum TaskMessage {
    /// Count query completed for a task.
    CountReady { index: usize, count: usize },
    /// A page was fetched for a task.
    PageFetched {
        index: usize,
        records_in_page: usize,
    },
    /// A task completed successfully.
    Completed { index: usize },
    /// A task failed.
    Failed { index: usize, error: String },
}

/// Modal for executing multiple OData queries in parallel with page-level progress.
///
/// Runs a `.count()` query per task first to get accurate totals, then paginates
/// all queries in parallel. Shows per-task progress and an overall ETA.
///
/// Fail-fast: if any task fails, all others are cancelled.
#[modal]
pub struct ODataFetchModal {
    /// Tasks to execute.
    #[state(skip)]
    tasks: Vec<ODataFetchTask>,

    /// Collected results per task (indexed by task order).
    #[state(skip)]
    results: Arc<std::sync::Mutex<Vec<Option<Vec<Record>>>>>,

    /// Per-task display info (reactive).
    task_infos: Vec<FetchTaskInfo>,

    /// Timestamp when fetching started (for ETA).
    #[state(skip)]
    start_time: Arc<std::sync::Mutex<Option<Instant>>>,

    /// ETA display string (reactive).
    eta: String,
}

impl ODataFetchModal {
    /// Create a new fetch modal with the given tasks.
    pub fn create(tasks: Vec<ODataFetchTask>) -> Self {
        let task_infos: Vec<FetchTaskInfo> = tasks
            .iter()
            .map(|t| FetchTaskInfo {
                label: t.label.clone(),
                ..Default::default()
            })
            .collect();
        let result_slots: Vec<Option<Vec<Record>>> = tasks.iter().map(|_| None).collect();

        Self::new(
            tasks,
            Arc::new(std::sync::Mutex::new(result_slots)),
            task_infos,
            Arc::new(std::sync::Mutex::new(None)),
            String::new(),
        )
    }
}

#[modal_impl(Result = Result<Vec<Vec<Record>>, ODataFetchError>)]
impl ODataFetchModal {
    fn default_result(&self) -> Result<Vec<Vec<Record>>, ODataFetchError> {
        Err(ODataFetchError::Cancelled)
    }

    #[on_start]
    async fn on_start(&self, mx: &ModalContext<Result<Vec<Vec<Record>>, ODataFetchError>>) {
        let task_count = self.tasks.len();

        if task_count == 0 {
            mx.close(Ok(Vec::new()));
            return;
        }

        // Record start time
        {
            let mut start = self.start_time.lock().unwrap();
            *start = Some(Instant::now());
        }

        let (tx, mut rx) = mpsc::unbounded_channel::<TaskMessage>();
        let cancel_token = CancellationToken::new();

        // Spawn all tasks
        let mut handles = Vec::new();

        for index in 0..task_count {
            let client = self.tasks[index].client.clone();
            let query = self.tasks[index].query.clone();
            let tx = tx.clone();
            let token = cancel_token.clone();
            let results = self.results.clone();

            // Mark as counting
            self.task_infos.update(|infos| {
                infos[index].status = FetchTaskStatus::Counting;
            });

            let handle = tokio::spawn(async move {
                // Phase 1: Count
                let count_query = query.clone();
                let count = tokio::select! {
                    _ = token.cancelled() => return,
                    result = count_query.count(&client) => {
                        match result {
                            Ok(c) => c,
                            Err(e) => {
                                let _ = tx.send(TaskMessage::Failed {
                                    index,
                                    error: e.to_string(),
                                });
                                return;
                            }
                        }
                    }
                };

                let _ = tx.send(TaskMessage::CountReady { index, count });

                // Phase 2: Paginate
                let mut pages = query.into_async_iter(&client);
                let mut all_records = Vec::new();

                loop {
                    let page_result = tokio::select! {
                        _ = token.cancelled() => return,
                        result = pages.next(&client) => result,
                    };

                    match page_result {
                        None => break, // No more pages
                        Some(Ok(page)) => {
                            let records_in_page = page.len();
                            all_records.extend(page.into_records());
                            let _ = tx.send(TaskMessage::PageFetched {
                                index,
                                records_in_page,
                            });
                        }
                        Some(Err(e)) => {
                            let _ = tx.send(TaskMessage::Failed {
                                index,
                                error: e.to_string(),
                            });
                            return;
                        }
                    }
                }

                // Store results
                {
                    let mut results_guard = results.lock().unwrap();
                    results_guard[index] = Some(all_records);
                }

                let _ = tx.send(TaskMessage::Completed { index });
            });

            handles.push(handle);
        }

        // Drop our sender so rx will close when all tasks finish
        drop(tx);

        // Process messages
        while let Some(msg) = rx.recv().await {
            match msg {
                TaskMessage::CountReady { index, count } => {
                    self.task_infos.update(|infos| {
                        infos[index].total_count = Some(count);
                        infos[index].status = FetchTaskStatus::Fetching;
                    });
                }
                TaskMessage::PageFetched {
                    index,
                    records_in_page,
                } => {
                    self.task_infos.update(|infos| {
                        infos[index].records_fetched += records_in_page;
                    });
                    // Update ETA
                    self.update_eta();
                }
                TaskMessage::Completed { index } => {
                    self.task_infos.update(|infos| {
                        infos[index].status = FetchTaskStatus::Completed;
                    });
                    self.update_eta();
                }
                TaskMessage::Failed { index, error } => {
                    let label = self.task_infos.get()[index].label.clone();

                    self.task_infos.update(|infos| {
                        infos[index].status = FetchTaskStatus::Failed;
                    });

                    // Fail-fast: cancel all other tasks
                    cancel_token.cancel();

                    // Mark remaining tasks as cancelled
                    self.task_infos.update(|infos| {
                        for info in infos.iter_mut() {
                            if info.status != FetchTaskStatus::Failed
                                && info.status != FetchTaskStatus::Completed
                            {
                                info.status = FetchTaskStatus::Cancelled;
                            }
                        }
                    });

                    // Abort all handles
                    for handle in &handles {
                        handle.abort();
                    }

                    mx.close(Err(ODataFetchError::TaskFailed { label, error }));
                    return;
                }
            }
        }

        // All tasks completed successfully — collect results
        let results_guard = self.results.lock().unwrap();
        let collected: Vec<Vec<Record>> = results_guard
            .iter()
            .map(|slot| slot.clone().unwrap_or_default())
            .collect();

        mx.close(Ok(collected));
    }

    fn element(&self) -> Element {
        let infos = self.task_infos.get();
        let completed = infos
            .iter()
            .filter(|t| t.status == FetchTaskStatus::Completed)
            .count();
        let total = infos.len();

        let eta_str = self.eta.get();
        let header = if eta_str.is_empty() {
            format!("Fetching data... ({}/{})", completed, total)
        } else {
            format!("Fetching data... ({}/{})  ~{}", completed, total, eta_str)
        };

        let task_rows: Vec<Element> = infos
            .iter()
            .enumerate()
            .map(|(idx, info)| self.render_task_row(idx, info))
            .collect();

        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                text (content: {header}) style (fg: primary, bold: true)
                column (gap: 0) {
                    ...task_rows
                }
            }
        }
    }
}

impl ODataFetchModal {
    fn update_eta(&self) {
        let start_time = {
            let guard = self.start_time.lock().unwrap();
            match *guard {
                Some(t) => t,
                None => return,
            }
        };

        let infos = self.task_infos.get();
        let total_fetched: usize = infos.iter().map(|i| i.records_fetched).sum();
        let total_expected: usize = infos.iter().filter_map(|i| i.total_count).sum();

        if total_fetched == 0 || total_expected == 0 {
            self.eta.set(String::new());
            return;
        }

        let elapsed = start_time.elapsed();
        let elapsed_secs = elapsed.as_secs_f64();

        if elapsed_secs < 0.5 {
            return; // Too early for meaningful ETA
        }

        let records_remaining = total_expected.saturating_sub(total_fetched);
        let rate = total_fetched as f64 / elapsed_secs; // records per second

        if rate < 0.001 {
            self.eta.set(String::new());
            return;
        }

        let remaining_secs = (records_remaining as f64 / rate).ceil() as u64;
        self.eta.set(format_duration(remaining_secs));
    }

    fn render_task_row(&self, idx: usize, info: &FetchTaskInfo) -> Element {
        let label = info.label.clone();
        let progress_text = format_progress(info);

        match info.status {
            FetchTaskStatus::Pending => {
                element! {
                    row (gap: 1, width: fill) {
                        text (content: "○") style (fg: muted)
                        text (content: {label}) style (fg: muted)
                        row (flex_grow: 1, justify: end) {
                            text (content: {progress_text}) style (fg: muted)
                        }
                    }
                }
            }
            FetchTaskStatus::Counting => {
                let spinner = BrailleSpinner::new()
                    .id(format!("task-spinner-{}", idx))
                    .build_standalone();
                element! {
                    row (gap: 1, width: fill) {
                        { spinner }
                        text (content: {label}) style (fg: primary)
                        row (flex_grow: 1, justify: end) {
                            text (content: "counting...") style (fg: muted)
                        }
                    }
                }
            }
            FetchTaskStatus::Fetching => {
                let spinner = BrailleSpinner::new()
                    .id(format!("task-spinner-{}", idx))
                    .build_standalone();
                element! {
                    row (gap: 1, width: fill) {
                        { spinner }
                        text (content: {label}) style (fg: primary)
                        row (flex_grow: 1, justify: end) {
                            text (content: {progress_text}) style (fg: muted)
                        }
                    }
                }
            }
            FetchTaskStatus::Completed => {
                element! {
                    row (gap: 1, width: fill) {
                        text (content: "●") style (fg: success)
                        text (content: {label}) style (fg: primary)
                        row (flex_grow: 1, justify: end) {
                            text (content: {progress_text}) style (fg: muted)
                        }
                    }
                }
            }
            FetchTaskStatus::Failed => {
                element! {
                    row (gap: 1, width: fill) {
                        text (content: "✗") style (fg: error)
                        text (content: {label}) style (fg: error)
                        row (flex_grow: 1, justify: end) {
                            text (content: {progress_text}) style (fg: muted)
                        }
                    }
                }
            }
            FetchTaskStatus::Cancelled => {
                element! {
                    row (gap: 1, width: fill) {
                        text (content: "⊘") style (fg: muted)
                        text (content: {label}) style (fg: muted)
                    }
                }
            }
        }
    }
}

/// Format a record count with thousand separators.
fn format_number(n: usize) -> String {
    let s = n.to_string();
    let mut result = String::with_capacity(s.len() + s.len() / 3);
    for (i, ch) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }
    result.chars().rev().collect()
}

/// Format progress text for a task.
fn format_progress(info: &FetchTaskInfo) -> String {
    match (&info.status, info.total_count) {
        (FetchTaskStatus::Pending, _) => String::new(),
        (FetchTaskStatus::Counting, _) => String::new(),
        (FetchTaskStatus::Cancelled, _) => String::new(),
        (_, Some(total)) => {
            format!(
                "{} / {} records",
                format_number(info.records_fetched),
                format_number(total)
            )
        }
        (_, None) => {
            format!("{} records", format_number(info.records_fetched))
        }
    }
}

/// Format a duration in seconds to a human-readable string.
fn format_duration(total_secs: u64) -> String {
    if total_secs == 0 {
        return "< 1s remaining".to_string();
    }

    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let seconds = total_secs % 60;

    let mut parts = Vec::new();

    if hours > 0 {
        parts.push(format!("{}h", hours));
    }
    if minutes > 0 {
        parts.push(format!("{}m", minutes));
    }
    if seconds > 0 || parts.is_empty() {
        parts.push(format!("{}s", seconds));
    }

    format!("{} remaining", parts.join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_number() {
        assert_eq!(format_number(0), "0");
        assert_eq!(format_number(999), "999");
        assert_eq!(format_number(1000), "1,000");
        assert_eq!(format_number(1234567), "1,234,567");
        assert_eq!(format_number(12345), "12,345");
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(0), "< 1s remaining");
        assert_eq!(format_duration(5), "5s remaining");
        assert_eq!(format_duration(60), "1m remaining");
        assert_eq!(format_duration(90), "1m 30s remaining");
        assert_eq!(format_duration(3661), "1h 1m 1s remaining");
        assert_eq!(format_duration(7200), "2h remaining");
    }
}
