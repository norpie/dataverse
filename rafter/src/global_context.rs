//! Global context for runtime-wide operations.
//!
//! GlobalContext provides access to global operations like:
//! - Instance management (spawn, close, focus)
//! - Toast notifications
//! - Theme changes
//! - Global modals
//! - Inter-app communication (publish, request)
//!
//! This is a stub - full implementation in 2.2d.

/// Global context for runtime-wide operations.
///
/// Passed to handlers that need global access. Systems receive this directly.
/// Apps receive it alongside AppContext when declared in handler signature.
#[derive(Clone)]
pub struct GlobalContext {
    // Stub - fields added in 2.2d
}

impl GlobalContext {
    /// Create a new global context (runtime use only).
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for GlobalContext {
    fn default() -> Self {
        Self::new()
    }
}
