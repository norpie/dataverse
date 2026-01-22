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

// =============================================================================
// Events
// =============================================================================

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
