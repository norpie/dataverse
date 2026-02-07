//! Procedural macros for dataverse-tui.

mod parallel_load;
mod settings;

use proc_macro::TokenStream;

/// Derives settings structure with automatic persistence.
///
/// Transforms fields into `Setting<T>` wrappers and generates loading logic.
///
/// # Example
///
/// ```rust,ignore
/// #[settings]
/// pub struct QueueSettings {
///     #[default = 5]
///     max_concurrency: usize,
/// }
/// ```
#[proc_macro_attribute]
pub fn settings(attr: TokenStream, item: TokenStream) -> TokenStream {
    settings::expand(attr, item)
}

/// Execute multiple async operations in parallel with progress display.
///
/// Shows a modal with the status of each task and returns typed results.
/// Supports optional fail-fast behavior to cancel remaining tasks on first failure.
///
/// # Syntax
///
/// ```rust,ignore
/// let (result1, result2) = parallel_load!(gx, {
///     "Label 1" => async_operation_1(),
///     "Label 2" => async_operation_2(),
/// }).await;
/// ```
///
/// # Options
///
/// - `fail_fast: bool` - Cancel remaining tasks on first failure (default: `true`)
///
/// ```rust,ignore
/// let (a, b, c) = parallel_load!(gx, fail_fast: false, {
///     "Task A" => do_a(),
///     "Task B" => do_b(),
///     "Task C" => do_c(),
/// }).await;
/// ```
///
/// # Return Value
///
/// Returns a tuple of `Result<T, ParallelLoadError>` for each task. The result is:
/// - `Ok(value)` if the task completed (the value itself may be a `Result`)
/// - `Err(ParallelLoadError::Cancelled { failed_task })` if the task was cancelled
///   due to fail-fast, with the label of the task that failed
/// - `Err(ParallelLoadError::Dropped)` if the task panicked or its channel was lost
///
/// # Example
///
/// ```rust,ignore
/// let (entities, attributes) = parallel_load!(gx, {
///     "Loading entities" => client.query(Entity::set("accounts")).execute(),
///     "Loading attributes" => client.metadata().attributes("account"),
/// });
///
/// let entities = entities.map_err(|e| log::warn!("{e}"))??;
/// let attributes = attributes.map_err(|e| log::warn!("{e}"))??;
/// ```
#[proc_macro]
pub fn parallel_load(input: TokenStream) -> TokenStream {
    parallel_load::expand(input)
}
