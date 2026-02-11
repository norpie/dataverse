//! Queue API types (requests and events).

use rafter::Event;
use rafter::Request;

use super::repository::StatusCounts;
use super::types::ItemStatus;
use super::types::QueueItemId;
use super::types::QueuePayload;

// =============================================================================
// Requests
// =============================================================================

/// Request to add items to the queue.
#[derive(Request)]
#[response(AddItemsResponse)]
pub struct AddItems {
    /// Items to add.
    pub items: Vec<NewItem>,
}

/// A new item to add to the queue.
pub struct NewItem {
    /// Priority (higher = more urgent).
    pub priority: i32,
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
}

/// Response to AddItems request.
pub struct AddItemsResponse {
    /// IDs of the newly created items.
    pub ids: Vec<QueueItemId>,
}

/// Request to get current queue status counts.
#[derive(Request)]
#[response(StatusCounts)]
pub struct GetQueueStatus;

/// Request to get operation results for a completed queue item.
#[derive(Request)]
#[response(GetItemResultsResponse)]
pub struct GetItemResults {
    /// The queue item ID to get results for.
    pub item_id: QueueItemId,
}

/// Response containing execution records and their operation results.
pub struct GetItemResultsResponse {
    /// Execution records with their per-operation results.
    pub executions: Vec<ExecutionWithResults>,
}

/// An execution record paired with its operation results.
pub struct ExecutionWithResults {
    /// The execution record.
    pub execution: super::types::ExecutionRecord,
    /// Per-operation results from this execution.
    pub results: Vec<super::types::OperationResultRecord>,
}

/// Request to pause the queue (stop picking up new items).
#[derive(Request)]
#[response(())]
pub struct PauseQueue;

/// Request to resume the queue (start picking up new items).
#[derive(Request)]
#[response(())]
pub struct ResumeQueue;

/// Request to delete specific queue items by ID.
/// Items that are currently running cannot be deleted.
#[derive(Request)]
#[response(DeleteItemsResponse)]
pub struct DeleteItems {
    /// IDs of items to delete.
    pub ids: Vec<QueueItemId>,
}

/// Response to DeleteItems request.
pub struct DeleteItemsResponse {
    /// Number of items actually deleted.
    pub deleted: usize,
}

/// Request to delete all non-running queue items with a specific source.
#[derive(Request)]
#[response(DeleteItemsResponse)]
pub struct DeleteItemsBySource {
    /// Source identifier to match.
    pub source: String,
}

// =============================================================================
// Events
// =============================================================================

/// Event published when queue system is initialized and ready.
#[derive(Clone, Event)]
pub struct QueueReady {
    /// Whether the queue is currently executing.
    pub is_running: bool,
    /// Current status counts.
    pub counts: StatusCounts,
}

/// Event published when queue status changes.
#[derive(Clone, Event)]
pub struct QueueStatusChanged {
    /// Whether the queue is currently executing.
    pub is_running: bool,
    /// Current status counts.
    pub counts: StatusCounts,
}

/// Event published when an item completes execution.
#[derive(Clone, Event)]
pub struct QueueItemCompleted {
    /// The item ID.
    pub item_id: QueueItemId,
    /// The resulting status.
    pub status: ItemStatus,
    /// Error message if failed.
    pub error: Option<String>,
}
