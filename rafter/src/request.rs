use std::any::Any;

/// Trait for request types with an associated response.
///
/// Use `#[derive(Request)]` with `#[response(Type)]` to implement this trait.
///
/// # Example
///
/// ```ignore
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
