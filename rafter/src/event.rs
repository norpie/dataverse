use std::any::Any;

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
