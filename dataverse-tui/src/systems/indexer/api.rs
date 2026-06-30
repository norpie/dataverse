//! Indexer API types (requests and events).

use std::fmt;

use chrono::{DateTime, Utc};
use dataverse_lib::api::query::odata::QUERY_CACHE_PREFIX;
use dataverse_lib::api::{
    CACHE_KEY_ALL_ENTITIES, CACHE_KEY_ALL_GLOBAL_OPTIONSETS, CACHE_KEY_ATTRIBUTE,
    CACHE_KEY_ATTRIBUTES, CACHE_KEY_ENTITY_CORE, CACHE_KEY_ENTITY_FULL, CACHE_KEY_GLOBAL_OPTIONSET,
    CACHE_KEY_RELATIONSHIP,
};
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
    /// Entity list cache TTL in hours.
    pub cache_entity_list_ttl_hours: u64,
    /// Entity metadata cache TTL in hours.
    pub cache_entity_metadata_ttl_hours: u64,
    /// Attribute metadata cache TTL in hours.
    pub cache_attribute_metadata_ttl_hours: u64,
    /// Global option set cache TTL in hours.
    pub cache_global_optionset_ttl_hours: u64,
    /// Relationship cache TTL in hours.
    pub cache_relationship_ttl_hours: u64,
    /// Query result cache TTL in hours.
    pub cache_query_ttl_hours: u64,
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
    /// Entity list cache TTL in hours.
    pub cache_entity_list_ttl_hours: u64,
    /// Entity metadata cache TTL in hours.
    pub cache_entity_metadata_ttl_hours: u64,
    /// Attribute metadata cache TTL in hours.
    pub cache_attribute_metadata_ttl_hours: u64,
    /// Global option set cache TTL in hours.
    pub cache_global_optionset_ttl_hours: u64,
    /// Relationship cache TTL in hours.
    pub cache_relationship_ttl_hours: u64,
    /// Query result cache TTL in hours.
    pub cache_query_ttl_hours: u64,
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
    /// Entity list cache TTL in hours.
    pub cache_entity_list_ttl_hours: u64,
    /// Entity metadata cache TTL in hours.
    pub cache_entity_metadata_ttl_hours: u64,
    /// Attribute metadata cache TTL in hours.
    pub cache_attribute_metadata_ttl_hours: u64,
    /// Global option set cache TTL in hours.
    pub cache_global_optionset_ttl_hours: u64,
    /// Relationship cache TTL in hours.
    pub cache_relationship_ttl_hours: u64,
    /// Query result cache TTL in hours.
    pub cache_query_ttl_hours: u64,
}

/// Event to open the indexer dashboard modal.
#[derive(Clone, Event)]
pub struct OpenIndexerDashboard;

/// Event to clear a specific cache category.
#[derive(Clone, Event)]
pub struct ClearCacheCategoryEvent {
    /// Which cache category to clear.
    pub category: CacheCategory,
    /// If true, clear across all cached environments instead of just the active one.
    pub all_environments: bool,
}

// =============================================================================
// Cache Categories
// =============================================================================

/// Categories of cached data that can be cleared independently.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheCategory {
    /// Entity list + entity core + entity full metadata.
    Entities,
    /// Single attribute + all attributes per entity.
    Attributes,
    /// Relationship metadata.
    Relationships,
    /// Global option set metadata (single + all).
    GlobalOptionSets,
    /// OData/FetchXML query result pages.
    Queries,
    /// All cached data.
    All,
}

impl CacheCategory {
    /// All selectable categories (excluding All).
    pub const INDIVIDUAL: &'static [CacheCategory] = &[
        CacheCategory::Entities,
        CacheCategory::Attributes,
        CacheCategory::Relationships,
        CacheCategory::GlobalOptionSets,
        CacheCategory::Queries,
    ];

    /// Returns the cache key prefixes associated with this category.
    pub fn prefixes(&self) -> &'static [&'static str] {
        match self {
            CacheCategory::Entities => &[
                CACHE_KEY_ALL_ENTITIES,
                CACHE_KEY_ENTITY_CORE,
                CACHE_KEY_ENTITY_FULL,
            ],
            CacheCategory::Attributes => &[CACHE_KEY_ATTRIBUTE, CACHE_KEY_ATTRIBUTES],
            CacheCategory::Relationships => &[CACHE_KEY_RELATIONSHIP],
            CacheCategory::GlobalOptionSets => {
                &[CACHE_KEY_ALL_GLOBAL_OPTIONSETS, CACHE_KEY_GLOBAL_OPTIONSET]
            }
            CacheCategory::Queries => &[QUERY_CACHE_PREFIX],
            CacheCategory::All => &[], // handled specially via cache.clear()
        }
    }
}

impl fmt::Display for CacheCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CacheCategory::Entities => write!(f, "Entities"),
            CacheCategory::Attributes => write!(f, "Attributes"),
            CacheCategory::Relationships => write!(f, "Relationships"),
            CacheCategory::GlobalOptionSets => write!(f, "Global Option Sets"),
            CacheCategory::Queries => write!(f, "Queries"),
            CacheCategory::All => write!(f, "All"),
        }
    }
}
