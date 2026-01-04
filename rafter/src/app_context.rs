//! App context for instance-specific operations.
//!
//! AppContext provides access to instance-specific state:
//! - Instance identity
//! - Widget state (trigger_widget_id, activated, selected, etc.)
//! - Focus within the app
//!
//! This is a stub - full implementation in 2.2d.

/// App context for instance-specific operations.
///
/// Passed to app handlers that need instance-specific access.
#[derive(Clone)]
pub struct AppContext {
    // Stub - fields added in 2.2d
}

impl AppContext {
    /// Create a new app context (runtime use only).
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for AppContext {
    fn default() -> Self {
        Self::new()
    }
}
