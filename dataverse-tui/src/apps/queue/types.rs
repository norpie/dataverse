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
            Self::Running => "warning",
            Self::Interrupted => "warning",
            Self::Done => "success",
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

/// Filter for which statuses to show in the tree view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum StatusFilter {
    /// Show all statuses.
    #[default]
    All,
    /// Show only items with this specific status.
    Only(ItemStatus),
}

impl StatusFilter {
    /// All possible filter values in cycle order.
    const CYCLE: &'static [Self] = &[
        Self::All,
        Self::Only(ItemStatus::Blocked),
        Self::Only(ItemStatus::Ready),
        Self::Only(ItemStatus::Paused),
        Self::Only(ItemStatus::Running),
        Self::Only(ItemStatus::Interrupted),
        Self::Only(ItemStatus::Done),
        Self::Only(ItemStatus::Failed),
        Self::Only(ItemStatus::PartiallyFailed),
    ];

    /// Advance to the next filter in the cycle.
    pub fn next(self) -> Self {
        let idx = Self::CYCLE.iter().position(|f| *f == self).unwrap_or(0);
        Self::CYCLE[(idx + 1) % Self::CYCLE.len()]
    }

    /// Display label for this filter.
    pub fn label(&self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Only(ItemStatus::Blocked) => "Blocked",
            Self::Only(ItemStatus::Ready) => "Ready",
            Self::Only(ItemStatus::Paused) => "Paused",
            Self::Only(ItemStatus::Running) => "Running",
            Self::Only(ItemStatus::Interrupted) => "Interrupted",
            Self::Only(ItemStatus::Done) => "Done",
            Self::Only(ItemStatus::Failed) => "Failed",
            Self::Only(ItemStatus::PartiallyFailed) => "Partial",
        }
    }

    /// Convert to a list of statuses for repository query.
    pub fn to_statuses(&self) -> Option<Vec<ItemStatus>> {
        match self {
            Self::All => None,
            Self::Only(status) => Some(vec![*status]),
        }
    }
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

/// Timing information for displaying elapsed/duration on queue items.
#[derive(Debug, Clone, Copy)]
pub enum ItemTiming {
    /// Currently running - show elapsed time since start.
    Running { started_at: DateTime<Utc> },
    /// Completed - show duration from last execution.
    Completed { duration_ms: i64 },
}

/// Result of a single operation within a batch execution.
#[derive(Debug, Clone)]
pub struct OperationResultRecord {
    /// The execution this result belongs to.
    pub execution_id: i64,
    /// Position in the batch (0-indexed).
    pub op_index: i32,
    /// Content-ID set on the operation (e.g., source record GUID).
    pub content_id: Option<String>,
    /// Whether the operation succeeded.
    pub success: bool,
    /// Type of operation (create, update, delete, etc.) - only for successes.
    pub operation_type: Option<String>,
    /// Additional result data as JSON (e.g., created ID) - only for successes.
    pub result_data: Option<String>,
    /// HTTP status code - only for failures.
    pub error_status: Option<i32>,
    /// Dataverse error code - only for failures.
    pub error_code: Option<String>,
    /// Error message - only for failures.
    pub error_message: Option<String>,
}
