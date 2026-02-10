//! Parallel loading modal for executing multiple async operations concurrently.
//!
//! Use via the `parallel_load!` macro for ergonomic syntax with typed results.

use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use rafter::prelude::*;
use rafter::widgets::Text;
use rafter::{element, page};
use tokio::sync::mpsc;

use crate::widgets::BrailleSpinner;

type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

/// Error returned when a parallel-loaded task did not produce a result.
#[derive(Debug, Clone)]
pub enum ParallelLoadError {
    /// This task was cancelled because another task failed (fail-fast).
    Cancelled {
        /// The label of the task whose failure triggered cancellation.
        failed_task: String,
    },
    /// The task was dropped (e.g. it panicked or the channel was lost).
    Dropped,
}

impl fmt::Display for ParallelLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cancelled { failed_task } => {
                write!(f, "Task cancelled: \"{failed_task}\" failed")
            }
            Self::Dropped => write!(f, "Task dropped (possibly panicked)"),
        }
    }
}

impl std::error::Error for ParallelLoadError {}

/// Trait to determine if an async result represents success or failure.
///
/// Used by `ParallelLoadingModal` to decide whether to short-circuit
/// when `fail_fast` is enabled.
///
/// # Example
///
/// ```ignore
/// // Already implemented for Result and Option
/// assert!(Ok::<_, ()>(42).is_success());
/// assert!(!Err::<(), _>("error").is_success());
/// assert!(Some(42).is_success());
/// assert!(!None::<i32>.is_success());
///
/// // Implement for custom types
/// impl Checkable for MyResponse {
///     fn is_success(&self) -> bool {
///         self.status_code < 400
///     }
/// }
/// ```
pub trait Checkable {
    fn is_success(&self) -> bool;
}

impl<T, E> Checkable for Result<T, E> {
    fn is_success(&self) -> bool {
        self.is_ok()
    }
}

impl<T> Checkable for Option<T> {
    fn is_success(&self) -> bool {
        self.is_some()
    }
}

/// Status of a parallel task.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TaskStatus {
    /// Task is waiting to start.
    #[default]
    Pending,
    /// Task is currently running.
    Running,
    /// Task completed successfully.
    Completed,
    /// Task failed.
    Failed,
    /// Task was cancelled due to another task failing.
    Cancelled,
}

/// A task to be executed in parallel.
///
/// Wraps an async operation with a label for display purposes.
/// The future must return a `bool` indicating success (`true`) or failure (`false`).
pub struct ParallelTask {
    /// Display label for this task.
    pub label: String,
    /// The future to execute (taken on start).
    pub future: Option<BoxFuture<bool>>,
}

impl ParallelTask {
    /// Create a new parallel task.
    ///
    /// The future should return `true` on success, `false` on failure.
    pub fn new<F>(label: impl Into<String>, future: F) -> Self
    where
        F: Future<Output = bool> + Send + 'static,
    {
        Self {
            label: label.into(),
            future: Some(Box::pin(future)),
        }
    }
}

/// Message sent when a task completes.
struct TaskComplete {
    index: usize,
    success: bool,
}

/// Display info for a task (cloneable for reactive state).
#[derive(Clone, Debug, Default)]
pub struct TaskInfo {
    pub label: String,
    pub status: TaskStatus,
}

/// Modal for executing multiple async operations in parallel with progress display.
///
/// Shows the status of each task (pending, running, completed, failed, cancelled)
/// and optionally short-circuits on first failure.
///
/// Use via the `parallel_load!` macro for ergonomic typed results.
#[modal]
pub struct ParallelLoadingModal {
    /// Tasks to execute (futures are taken on start).
    #[state(skip)]
    tasks: Arc<Mutex<Vec<ParallelTask>>>,

    /// Whether to cancel remaining tasks on first failure.
    #[state(skip)]
    fail_fast: bool,

    /// Label of the task that caused fail-fast cancellation (shared with macro).
    #[state(skip)]
    failed_task_label: Arc<Mutex<Option<String>>>,

    /// Task display info (reactive for UI updates).
    task_infos: Vec<TaskInfo>,
}

impl ParallelLoadingModal {
    /// Set whether to cancel remaining tasks on first failure.
    ///
    /// Default is `true`.
    pub fn fail_fast(mut self, fail_fast: bool) -> Self {
        self.fail_fast = fail_fast;
        self
    }
}

#[modal_impl(Result = ())]
impl ParallelLoadingModal {
    fn default_result(&self) {}

    #[on_start]
    async fn on_start(&self, mx: &ModalContext<()>) {
        let (tx, mut rx) = mpsc::unbounded_channel::<TaskComplete>();

        // Take and spawn all tasks
        let mut handles = Vec::new();
        {
            let mut tasks_guard = self.tasks.lock().unwrap();
            for (index, task) in tasks_guard.iter_mut().enumerate() {
                if let Some(future) = task.future.take() {
                    // Update status to running
                    self.task_infos.update(|infos| {
                        infos[index].status = TaskStatus::Running;
                    });

                    let tx = tx.clone();
                    let handle = tokio::spawn(async move {
                        let success = future.await;
                        let _ = tx.send(TaskComplete { index, success });
                    });
                    handles.push(Some(handle));
                } else {
                    handles.push(None);
                }
            }
        }

        // Drop sender so rx completes when all tasks are done
        drop(tx);

        // If no tasks, close immediately
        if handles.is_empty() {
            mx.close(());
            return;
        }

        let fail_fast = self.fail_fast;
        let mut short_circuited = false;

        // Process completions
        while let Some(complete) = rx.recv().await {
            let failed = !complete.success;

            // Update task status
            self.task_infos.update(|infos| {
                infos[complete.index].status = if complete.success {
                    TaskStatus::Completed
                } else {
                    TaskStatus::Failed
                };
            });

            // Short-circuit if enabled and task failed
            if fail_fast && failed && !short_circuited {
                short_circuited = true;

                // Record which task caused the cancellation
                let infos = self.task_infos.get();
                {
                    let mut label = self.failed_task_label.lock().unwrap();
                    if label.is_none() {
                        *label = Some(infos[complete.index].label.clone());
                    }
                }

                // Cancel remaining running tasks
                for (i, info) in infos.iter().enumerate() {
                    if info.status == TaskStatus::Running {
                        if let Some(handle) = handles.get_mut(i).and_then(|h| h.take()) {
                            handle.abort();
                        }
                        self.task_infos.update(|infos| {
                            infos[i].status = TaskStatus::Cancelled;
                        });
                    }
                }
            }
        }

        mx.close(());
    }

    fn element(&self) -> Element {
        let infos = self.task_infos.get();
        let completed = infos
            .iter()
            .filter(|t| t.status == TaskStatus::Completed)
            .count();
        let total = infos.len();
        let header = format!("Loading... ({}/{})", completed, total);

        let task_rows: Vec<Element> = infos
            .iter()
            .enumerate()
            .map(|(idx, info)| {
                let label = info.label.clone();
                let (indicator, label_elem): (Element, Element) = match info.status {
                    TaskStatus::Pending => (
                        element! { text (content: "○") style (fg: muted) },
                        element! { text (content: {label}) style (fg: muted) },
                    ),
                    TaskStatus::Running => (
                        BrailleSpinner::new()
                            .id(format!("task-spinner-{}", idx))
                            .build_standalone(),
                        element! { text (content: {label}) style (fg: primary) },
                    ),
                    TaskStatus::Completed => (
                        element! { text (content: "✓") style (fg: primary) },
                        element! { text (content: {label}) style (fg: primary) },
                    ),
                    TaskStatus::Failed => (
                        element! { text (content: "✗") style (fg: error) },
                        element! { text (content: {label}) style (fg: error) },
                    ),
                    TaskStatus::Cancelled => (
                        element! { text (content: "⊘") style (fg: muted) },
                        element! { text (content: {label}) style (fg: muted) },
                    ),
                };

                element! {
                    row (gap: 1) {
                        { indicator }
                        { label_elem }
                    }
                }
            })
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
