//! Instance types for multi-app runtime.

use std::time::Instant;

use uuid::Uuid;

/// Unique identifier for a running app instance.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct InstanceId(Uuid);

impl InstanceId {
    /// Create a new unique instance ID.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Get the underlying UUID.
    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl Default for InstanceId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for InstanceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Metadata about a running app instance.
///
/// Used for app launchers/switchers to display and select instances.
#[derive(Debug, Clone)]
pub struct InstanceInfo {
    /// Unique identifier for this instance.
    pub id: InstanceId,
    /// App type name (from config or type name).
    pub app_name: &'static str,
    /// Instance-specific title (e.g., "Record #123").
    pub title: String,
    /// When this instance was created.
    pub created_at: Instant,
    /// When this instance was last focused.
    pub last_focused_at: Instant,
    /// Whether this instance is currently focused.
    pub is_focused: bool,
}

impl InstanceInfo {
    /// Create new instance info.
    pub fn new(id: InstanceId, app_name: &'static str, title: String) -> Self {
        let now = Instant::now();
        Self {
            id,
            app_name,
            title,
            created_at: now,
            last_focused_at: now,
            is_focused: false,
        }
    }
}
