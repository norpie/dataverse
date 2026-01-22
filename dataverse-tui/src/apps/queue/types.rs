//! Queue data types.

use chrono::DateTime;
use chrono::Utc;
use dataverse_lib::api::Batch;
use dataverse_lib::api::Operation;
use serde::Deserialize;
use serde::Serialize;

/// Unique identifier for a queue item.
pub type QueueItemId = i64;

/// Status of a queue item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ItemStatus {
    /// Can't execute (environment doesn't exist).
    Blocked,
    /// Ready to execute.
    Ready,
    /// User manually paused this item.
    Paused,
    /// Currently executing.
    Running,
    /// Was running when app shut down, needs manual review.
    Interrupted,
    /// Completed successfully.
    Done,
    /// All operations failed.
    Failed,
    /// Some operations succeeded, some failed (batch only).
    PartiallyFailed,
}

impl ItemStatus {
    /// Returns true if this status represents a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Done | Self::Failed | Self::PartiallyFailed)
    }

    /// Returns true if this item can be executed.
    pub fn is_executable(&self) -> bool {
        matches!(self, Self::Ready)
    }

    /// Returns the display color for this status.
    pub fn color(&self) -> &'static str {
        match self {
            Self::Blocked => "muted",
            Self::Ready => "primary",
            Self::Paused => "warning",
            Self::Running => "success",
            Self::Interrupted => "warning",
            Self::Done => "primary",
            Self::Failed => "error",
            Self::PartiallyFailed => "warning",
        }
    }
}

/// The payload to execute - either a single operation or a batch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QueuePayload {
    /// A single CRUD operation.
    Single(Operation),
    /// A pre-constructed batch of operations.
    Batch(Batch),
}

impl QueuePayload {
    /// Returns the number of operations in this payload.
    pub fn operation_count(&self) -> usize {
        match self {
            Self::Single(_) => 1,
            Self::Batch(batch) => batch.operation_count(),
        }
    }
}

/// A queue item representing work to be done.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueItem {
    /// Unique identifier (database-assigned).
    pub id: QueueItemId,
    /// Priority (higher = more urgent).
    pub priority: i32,
    /// Current status.
    pub status: ItemStatus,
    /// The operation(s) to execute.
    pub payload: QueuePayload,
    /// Target environment ID.
    pub env_id: i64,
    /// Account ID for authentication.
    pub account_id: i64,
    /// Source identifier (e.g., "import", "sync", "manual").
    pub source: String,
    /// Human-readable description.
    pub description: String,
    /// When the item was created.
    pub created_at: DateTime<Utc>,
}

/// Status of an execution attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionStatus {
    /// All operations succeeded.
    Success,
    /// All operations failed.
    Failed,
    /// Some operations succeeded, some failed.
    PartialSuccess,
}

/// Record of an execution attempt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionRecord {
    /// Unique identifier (database-assigned).
    pub id: i64,
    /// The queue item that was executed.
    pub item_id: QueueItemId,
    /// When execution started.
    pub started_at: DateTime<Utc>,
    /// When execution completed.
    pub completed_at: DateTime<Utc>,
    /// Duration in milliseconds.
    pub duration_ms: i64,
    /// Overall status.
    pub status: ExecutionStatus,
    /// Error message if failed.
    pub error: Option<String>,
    /// Number of successful operations.
    pub success_count: i32,
    /// Number of failed operations.
    pub failure_count: i32,
}
