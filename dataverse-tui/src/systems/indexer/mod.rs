//! Metadata indexer system.
//!
//! Keeps metadata cache hot by periodically re-fetching before TTL expiry.
//! Runs as a background system with a dashboard modal for monitoring.

pub mod api;
mod migrations;
pub mod repository;
pub mod sync;

pub use api::*;
pub use repository::{EnvSync, IndexerRepository, RepositoryError, SyncLogEntry, SyncStatus};
pub use sync::{
    execute_task, get_check_tasks, SyncError, SyncTask, DEFAULT_CHECK_INTERVAL_SECS,
    DEFAULT_REFRESH_THRESHOLD_PCT,
};

// System implementation will be added in Phase 5.
