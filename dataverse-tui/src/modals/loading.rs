//! Generic loading modal for executing multiple async tasks with progress UI.
//!
//! Shows a checkbox list with live progress indicators for each task:
//! - ☐ Idle (shouldn't appear in practice)
//! - ⠋ Loading (animated braille spinner)
//! - ✓ Success (green checkmark)
//! - ✗ Error (red cross)

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{Button, Text};
use thiserror::Error;
use tokio_util::sync::CancellationToken;

use crate::widgets::BrailleSpinner;

type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
type TaskFactory = Box<dyn FnOnce() -> BoxFuture<'static, Result<(), String>> + Send>;

/// A task to be executed with progress tracking.
///
/// Tasks execute async operations and store results via captured variables
/// (side-effect pattern). This allows heterogeneous return types without
/// complex type gymnastics.
///
/// # Example
///
/// ```ignore
/// let mut data = Vec::new();
/// let client_clone = client.clone();
///
/// let task = LoadingTask::new("Loading data", move || async move {
///     let result = client_clone.fetch_data().await
///         .map_err(|e| e.to_string())?;
///     data = result;
///     Ok(())
/// });
/// ```
pub struct LoadingTask {
    /// Display label for this task.
    pub label: String,
    /// Factory function that creates the future when called.
    /// Uses Mutex for thread-safe interior mutability so we can consume it in on_start.
    factory: Mutex<Option<TaskFactory>>,
}

impl LoadingTask {
    /// Create a new loading task.
    ///
    /// The closure `f` should return a future that performs the async work.
    /// The future should capture any needed data and store results in
    /// external variables.
    ///
    /// # Example
    ///
    /// ```ignore
    /// LoadingTask::new("Fetching users", || async {
    ///     let users = api.get_users().await?;
    ///     user_list.extend(users);
    ///     Ok(())
    /// })
    /// ```
    pub fn new<F, Fut>(label: impl Into<String>, f: F) -> Self
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = Result<(), String>> + Send + 'static,
    {
        let factory: TaskFactory = Box::new(move || Box::pin(f()));
        Self {
            label: label.into(),
            factory: Mutex::new(Some(factory)),
        }
    }
}

/// Error type for loading modal failures.
#[derive(Debug, Clone, Error)]
pub enum LoadingError {
    /// One or more tasks failed.
    #[error("Task '{task_name}' failed: {message}")]
    TaskFailed {
        task_name: String,
        task_index: usize,
        message: String,
    },
}

/// Current state of the loading modal.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum LoadingState {
    #[default]
    Loading,
    Completed,
    Failed,
}

/// Loading modal that executes multiple tasks in parallel with progress UI.
///
/// # Example
///
/// ```ignore
/// let mut entities = Vec::new();
/// let mut attributes = Vec::new();
///
/// let client1 = client.clone();
/// let client2 = client.clone();
///
/// let result = gx.modal(LoadingModal::new(vec![
///     LoadingTask::new("Loading entities", move || async move {
///         entities = client1.metadata().all_entities().await
///             .map_err(|e| e.to_string())?;
///         Ok(())
///     }),
///     LoadingTask::new("Loading attributes", move || async move {
///         attributes = client2.metadata().attributes(entity).await
///             .map_err(|e| e.to_string())?;
///         Ok(())
///     }),
/// ]).shortcircuit()).await;
///
/// match result {
///     Ok(()) => {
///         // All tasks succeeded, entities and attributes are populated
///     }
///     Err(e) => {
///         gx.toast(Toast::error(e.to_string()));
///     }
/// }
/// ```
#[modal]
pub struct LoadingModal {
    #[state(skip)]
    tasks: Arc<Mutex<Option<Vec<LoadingTask>>>>,

    #[state(skip)]
    shortcircuit_mode: bool,

    /// Task labels (extracted from tasks before consumption).
    #[state(skip)]
    task_labels: Vec<String>,

    /// Resource state for each task.
    #[state(skip)]
    task_states: Vec<Resource<()>>,

    /// Overall modal state.
    state: LoadingState,

    /// Error message if shortcircuit triggered.
    error_message: Option<String>,

    /// Index of the task that failed (if any).
    failed_task_index: Option<usize>,

    /// Cancellation token for shortcircuit mode.
    cancel_token: Option<CancellationToken>,
}

impl LoadingModal {
    /// Create a new loading modal with the given tasks.
    ///
    /// Tasks will execute in parallel. The modal will close automatically
    /// when all tasks complete.
    pub fn new(tasks: Vec<LoadingTask>) -> Self {
        let task_count = tasks.len();
        let task_labels: Vec<String> = tasks.iter().map(|t| t.label.clone()).collect();
        let task_states: Vec<Resource<()>> = (0..task_count).map(|_| Resource::new()).collect();
        Self {
            tasks: Arc::new(Mutex::new(Some(tasks))),
            shortcircuit_mode: false,
            task_labels,
            task_states,
            ..Default::default()
        }
    }

    /// Enable shortcircuit mode.
    ///
    /// If any task fails, all remaining tasks will be cancelled and the
    /// modal will show an error UI with retry/cancel options.
    pub fn shortcircuit(mut self) -> Self {
        self.shortcircuit_mode = true;
        self
    }
}

#[modal_impl]
impl LoadingModal {
    fn default_result(&self) -> Result<(), LoadingError> {
        // Default to error if modal closes during shutdown
        Err(LoadingError::TaskFailed {
            task_name: "Unknown".to_string(),
            task_index: 0,
            message: "Modal closed unexpectedly".to_string(),
        })
    }

    #[on_start]
    async fn execute_tasks(&self, mx: &ModalContext<Result<(), LoadingError>>) {
        let cancel_token = CancellationToken::new();
        self.cancel_token.set(Some(cancel_token.clone()));

        // Take tasks (consumes them)
        let tasks = self.tasks.lock().unwrap().take();
        let Some(tasks) = tasks else {
            mx.close(Err(LoadingError::TaskFailed {
                task_name: "Error".to_string(),
                task_index: 0,
                message: "Tasks already consumed".to_string(),
            }));
            return;
        };

        // Spawn all tasks
        let mut handles = Vec::new();

        for (idx, task) in tasks.iter().enumerate() {
            // Take the factory out of the Mutex (consumes it)
            let factory = task
                .factory
                .lock()
                .unwrap()
                .take()
                .expect("Task factory already consumed");

            let resource = self.task_states[idx].clone();
            let task_name = task.label.clone();
            let task_cancel = cancel_token.clone();

            let handle = tokio::spawn(async move {
                resource.set_loading();

                // Create and execute the future
                let future = factory();

                // Run future with cancellation support
                let result = tokio::select! {
                    result = future => result,
                    _ = task_cancel.cancelled() => {
                        return Err((task_name.clone(), "Task cancelled".to_string()));
                    }
                };

                match result {
                    Ok(()) => {
                        resource.set_ready(());
                        Ok(())
                    }
                    Err(e) => {
                        resource.set_error(e.clone());
                        Err((task_name, e))
                    }
                }
            });

            handles.push((idx, handle));
        }

        // Wait for all tasks
        let mut first_error: Option<(usize, String, String)> = None;

        for (idx, handle) in handles {
            match handle.await {
                Ok(Ok(())) => {
                    // Task succeeded
                }
                Ok(Err((task_name, error_msg))) => {
                    // Task failed
                    if first_error.is_none() {
                        first_error = Some((idx, task_name, error_msg));
                    }

                    if self.shortcircuit_mode {
                        // Cancel all remaining tasks
                        cancel_token.cancel();

                        // Show error UI
                        let (idx, name, msg) = first_error.as_ref().unwrap();
                        self.failed_task_index.set(Some(*idx));
                        self.error_message.set(Some(format!("{}: {}", name, msg)));
                        self.state.set(LoadingState::Failed);

                        // Don't close - let user retry or cancel
                        return;
                    }
                }
                Err(e) => {
                    // Task panicked
                    let error_msg = format!("Task panicked: {}", e);
                    if first_error.is_none() {
                        first_error = Some((idx, "Task".to_string(), error_msg.clone()));
                    }

                    if self.shortcircuit_mode {
                        cancel_token.cancel();
                        self.failed_task_index.set(Some(idx));
                        self.error_message.set(Some(error_msg));
                        self.state.set(LoadingState::Failed);
                        return;
                    }
                }
            }
        }

        // All tasks completed
        self.state.set(LoadingState::Completed);

        // Check if any failed
        if let Some((idx, task_name, message)) = first_error {
            mx.close(Err(LoadingError::TaskFailed {
                task_name,
                task_index: idx,
                message,
            }));
        } else {
            mx.close(Ok(()));
        }
    }

    #[keybinds]
    fn keys() {
        bind("escape", cancel);
    }

    #[handler]
    async fn cancel(&self, mx: &ModalContext<Result<(), LoadingError>>) {
        // Cancel all tasks
        if let Some(token) = self.cancel_token.get() {
            token.cancel();
        }

        mx.close(Err(LoadingError::TaskFailed {
            task_name: "Cancelled".to_string(),
            task_index: 0,
            message: "User cancelled".to_string(),
        }));
    }

    #[handler]
    async fn retry(&self, mx: &ModalContext<Result<(), LoadingError>>) {
        // Reset state and restart
        self.error_message.set(None);
        self.failed_task_index.set(None);
        self.state.set(LoadingState::Loading);

        // Clear all task states
        for resource in &self.task_states {
            resource.set_idle();
        }

        // Restart execution
        self.execute_tasks(mx).await;
    }

    fn element(&self) -> Element {
        match self.state.get() {
            LoadingState::Loading | LoadingState::Completed => self.render_loading(),
            LoadingState::Failed => self.render_error(),
        }
    }

    fn render_loading(&self) -> Element {
        let task_count = self.task_states.len();
        
        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                text (content: "Loading") style (bold, fg: interact)

                // Task list
                column (gap: 1) {
                    for idx in 0..task_count {
                        { self.render_task_row_by_index(idx) }
                    }
                }
            }
        }
    }

    fn render_error(&self) -> Element {
        let error_msg = self
            .error_message
            .get()
            .unwrap_or_else(|| "Unknown error".to_string());

        let task_count = self.task_states.len();

        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                text (content: "Loading Failed") style (bold, fg: error)

                // Show task list with current states
                column (gap: 1) {
                    for idx in 0..task_count {
                        { self.render_task_row_by_index(idx) }
                    }
                }

                // Error message
                column (gap: 1, padding: (1, 2)) style (bg: background) {
                    text (content: error_msg) style (fg: error)
                }

                // Buttons
                row (width: fill, justify: between) {
                    button (label: "Cancel", hint: "esc", id: "cancel") on_activate: cancel()
                    button (label: "Retry", id: "retry") on_activate: retry()
                }
            }
        }
    }

    fn render_task_row_by_index(&self, idx: usize) -> Element {
        let state = self.task_states[idx].get();
        let label = self.task_labels.get(idx)
            .cloned()
            .unwrap_or_else(|| format!("Task {}", idx + 1));

        match state {
            ResourceState::Idle => {
                page! {
                    row (gap: 1) {
                        text (content: "☐") style (fg: muted)
                        text (content: label)
                    }
                }
            }
            ResourceState::Loading | ResourceState::Progress(_) => {
                let spinner_id = format!("task-{}-spinner", idx);
                page! {
                    row (gap: 1) {
                        braille_spinner (id: spinner_id)
                        text (content: label)
                    }
                }
            }
            ResourceState::Ready(_) => {
                page! {
                    row (gap: 1) {
                        text (content: "✓") style (fg: success)
                        text (content: label)
                    }
                }
            }
            ResourceState::Error(_) => {
                page! {
                    row (gap: 1) {
                        text (content: "✗") style (fg: error)
                        text (content: label)
                    }
                }
            }
        }
    }
}
