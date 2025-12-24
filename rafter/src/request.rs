use std::any::Any;

use crate::app::InstanceId;

/// Trait for request types with an associated response.
///
/// Use `#[derive(Request)]` with `#[response(Type)]` to implement this trait.
///
/// # Example
///
/// ```rust
/// use rafter::prelude::*;
///
/// #[derive(Request)]
/// #[response(bool)]
/// struct IsPaused;
///
/// #[derive(Request)]
/// #[response(Option<String>)]
/// struct GetUserName {
///     user_id: u64,
/// }
/// ```
pub trait Request: Send + Sync + Any + 'static {
    type Response: Send + Sync + 'static;
}

/// Error type for request failures.
#[derive(Debug, Clone, thiserror::Error)]
pub enum RequestError {
    /// No instance of the target app type is running.
    #[error("no instance of target app type is running")]
    NoInstance,

    /// The specified instance ID was not found.
    #[error("instance not found: {0}")]
    InstanceNotFound(InstanceId),

    /// The target instance is sleeping and cannot process requests.
    #[error("instance is sleeping: {0}")]
    InstanceSleeping(InstanceId),

    /// The target app has no handler for this request type.
    #[error("target app has no handler for this request type")]
    NoHandler,

    /// The request handler panicked during execution.
    #[error("request handler panicked")]
    HandlerPanicked,
}
