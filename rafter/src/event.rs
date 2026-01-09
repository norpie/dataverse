use std::any::Any;

use crate::instance::InstanceId;

/// Marker trait for publishable events.
///
/// Derive this trait to make a type publishable via `cx.publish()`.
/// Events are delivered to all subscribers, so they must be `Clone`.
///
/// # Example
///
/// ```ignore
/// use rafter::prelude::*;
///
/// #[derive(Event, Clone)]
/// struct UserLoggedIn {
///     user_id: u64,
/// }
/// ```
pub trait Event: Clone + Send + Sync + Any + 'static {}

// =============================================================================
// Runtime Events
// =============================================================================

/// Published when a new app instance is spawned.
#[derive(Debug, Clone)]
pub struct InstanceSpawned {
    /// The ID of the spawned instance.
    pub id: InstanceId,
    /// The name of the app.
    pub name: &'static str,
}

impl Event for InstanceSpawned {}

/// Published when an app instance is closed.
#[derive(Debug, Clone)]
pub struct InstanceClosed {
    /// The ID of the closed instance.
    pub id: InstanceId,
    /// The name of the app.
    pub name: &'static str,
}

impl Event for InstanceClosed {}

/// Published when focus changes between instances.
#[derive(Debug, Clone)]
pub struct FocusChanged {
    /// The previously focused instance (if any).
    pub old: Option<InstanceId>,
    /// The newly focused instance.
    pub new: InstanceId,
}

impl Event for FocusChanged {}
