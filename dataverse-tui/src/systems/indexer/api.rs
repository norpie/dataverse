//! Indexer API types (requests and events).

use chrono::{DateTime, Utc};
use rafter::Event;
use rafter::Request;

use super::repository::SyncLogEntry;
use crate::systems::taskbar::StatusIndicator;

// =============================================================================
// Requests
// =============================================================================

/// Request to get current indexer status.
#[derive(Request)]
#[response(IndexerStatusResponse)]
pub struct GetIndexerStatus;

/// Request to pause the indexer.
#[derive(Request)]
#[response(())]
pub struct PauseIndexer;

/// Request to resume the indexer.
#[derive(Request)]
#[response(())]
pub struct ResumeIndexer;

/// Request to trigger a sync for a specific environment or all environments.
#[derive(Request)]
#[response(())]
pub struct TriggerSync {
    /// Environment ID to sync, or None for all environments.
    pub env_id: Option<i64>,
}

/// Request to clear cache for a specific environment or all environments.
#[derive(Request)]
#[response(())]
pub struct ClearCache {
    /// Environment ID to clear cache for, or None for all environments.
    pub env_id: Option<i64>,
}

/// Request to get recent sync log entries.
#[derive(Request)]
#[response(Vec<SyncLogEntry>)]
pub struct GetSyncLogs {
    /// Environment ID to filter by, or None for all environments.
    pub env_id: Option<i64>,
    /// Maximum number of entries to return.
    pub limit: usize,
}

/// Request to get current indexer settings.
#[derive(Request)]
#[response(SyncSettings)]
pub struct GetIndexerSettings;

/// Request to update indexer settings.
#[derive(Request)]
#[response(())]
pub struct UpdateIndexerSettings {
    /// Check interval in seconds.
    pub check_interval_secs: u64,
    /// Refresh threshold as percentage of TTL (0-100).
    pub refresh_threshold_pct: u64,
}

// =============================================================================
// Responses
// =============================================================================

/// Response for GetIndexerStatus.
#[derive(Debug, Clone, Default)]
pub struct IndexerStatusResponse {
    /// Whether the indexer is paused.
    pub is_paused: bool,
    /// Overall status indicator.
    pub overall_status: StatusIndicator,
    /// Per-environment sync status.
    pub environments: Vec<EnvSyncStatus>,
}

/// Per-environment sync status.
#[derive(Debug, Clone)]
pub struct EnvSyncStatus {
    /// Environment ID.
    pub env_id: i64,
    /// Environment name.
    pub env_name: String,
    /// Current sync status.
    pub status: StatusIndicator,
    /// Last successful sync time.
    pub last_sync: Option<DateTime<Utc>>,
    /// Last error message if in error state.
    pub error: Option<String>,
    /// Current sync progress.
    pub progress: Option<SyncProgress>,
}

/// Per-environment sync progress tracking.
#[derive(Debug, Clone, Default)]
pub struct SyncProgress {
    /// Total entities to fetch metadata for.
    pub entities_total: u32,
    /// Entities fetched so far.
    pub entities_done: u32,
    /// Whether option sets fetch is pending.
    pub optionsets_pending: bool,
    /// Whether option sets fetch is complete.
    pub optionsets_done: bool,
}

/// Indexer sync settings.
#[derive(Debug, Clone, Default)]
pub struct SyncSettings {
    /// How often to check for near-expiry cache entries, in seconds.
    pub check_interval_secs: u64,
    /// Percentage of TTL elapsed before triggering a refresh (0-100).
    pub refresh_threshold_pct: u64,
}

// =============================================================================
// Events (outbound - published by system)
// =============================================================================

/// Event published when indexer is initialized and ready.
#[derive(Clone, Event)]
pub struct IndexerReady {
    /// Overall status indicator.
    pub overall_status: StatusIndicator,
    /// Per-environment sync status.
    pub environments: Vec<EnvSyncStatus>,
}

/// Event published when indexer status changes.
#[derive(Clone, Event)]
pub struct IndexerStatusChanged {
    /// Whether the indexer is paused.
    pub is_paused: bool,
    /// Overall status indicator.
    pub overall_status: StatusIndicator,
    /// Per-environment sync status.
    pub environments: Vec<EnvSyncStatus>,
}

/// Event published when indexer settings change.
#[derive(Clone, Event)]
pub struct IndexerSettingsChanged {
    /// Current settings.
    pub settings: SyncSettings,
}

// =============================================================================
// Events (inbound - handled by system)
// =============================================================================

/// Event to pause the indexer.
#[derive(Clone, Event)]
pub struct PauseIndexerEvent;

/// Event to resume the indexer.
#[derive(Clone, Event)]
pub struct ResumeIndexerEvent;

/// Event to trigger a sync.
#[derive(Clone, Event)]
pub struct TriggerSyncEvent {
    /// Environment ID to sync, or None for all environments.
    pub env_id: Option<i64>,
}

/// Event to update indexer settings.
#[derive(Clone, Event)]
pub struct UpdateIndexerSettingsEvent {
    /// Check interval in seconds.
    pub check_interval_secs: u64,
    /// Refresh threshold as percentage of TTL (0-100).
    pub refresh_threshold_pct: u64,
}

/// Event to open the indexer dashboard modal.
#[derive(Clone, Event)]
pub struct OpenIndexerDashboard;
