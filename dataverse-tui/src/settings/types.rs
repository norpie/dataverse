//! Settings type definitions.

use std::sync::Arc;

use crate::apps::queue::types::StatusFilter;

use super::{Setting, SettingsBackend, SettingsError};

/// Top-level settings structure.
pub struct Settings {
    pub indexer: IndexerSettings,
    pub queue: QueueSettings,
}

impl Settings {
    /// Load all settings from the backend.
    pub async fn load(backend: Arc<dyn SettingsBackend>) -> Result<Self, SettingsError> {
        Ok(Self {
            indexer: IndexerSettings::load(backend.clone()).await?,
            queue: QueueSettings::load(backend.clone()).await?,
        })
    }
}

/// Indexer system settings.
pub struct IndexerSettings {
    /// How often to check for stale metadata (in seconds).
    pub check_interval_secs: Setting<u64>,

    /// When to refresh metadata (percentage of TTL).
    pub refresh_threshold_pct: Setting<u64>,

    /// Whether the indexer is currently paused.
    pub is_paused: Setting<bool>,
}

impl IndexerSettings {
    async fn load(backend: Arc<dyn SettingsBackend>) -> Result<Self, SettingsError> {
        Ok(Self {
            check_interval_secs: Setting::load(
                backend.clone(),
                "Settings.Indexer.CheckIntervalSecs",
                60,
            )
            .await?,
            refresh_threshold_pct: Setting::load(
                backend.clone(),
                "Settings.Indexer.RefreshThresholdPct",
                80,
            )
            .await?,
            is_paused: Setting::load(backend.clone(), "Settings.Indexer.IsPaused", false).await?,
        })
    }
}

/// Queue app settings.
pub struct QueueSettings {
    /// Maximum number of concurrent operations.
    pub max_concurrency: Setting<usize>,

    /// Maximum consecutive failures before auto-pause.
    pub max_failures: Setting<usize>,

    /// Active status filter.
    pub status_filter: Setting<StatusFilter>,

    /// Source filter (multi-select).
    pub source_filter: Setting<Vec<String>>,

    /// Search text for filtering by description.
    pub search_text: Setting<String>,
}

impl QueueSettings {
    async fn load(backend: Arc<dyn SettingsBackend>) -> Result<Self, SettingsError> {
        Ok(Self {
            max_concurrency: Setting::load(backend.clone(), "Settings.Queue.MaxConcurrency", 5)
                .await?,
            max_failures: Setting::load(backend.clone(), "Settings.Queue.MaxFailures", 10).await?,
            status_filter: Setting::load(
                backend.clone(),
                "Settings.Queue.StatusFilter",
                StatusFilter::All,
            )
            .await?,
            source_filter: Setting::load(
                backend.clone(),
                "Settings.Queue.SourceFilter",
                Vec::new(),
            )
            .await?,
            search_text: Setting::load(backend.clone(), "Settings.Queue.SearchText", String::new())
                .await?,
        })
    }
}
