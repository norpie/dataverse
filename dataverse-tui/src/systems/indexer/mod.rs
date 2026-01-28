//! Metadata indexer system.
//!
//! Keeps metadata cache hot by periodically re-fetching before TTL expiry.
//! Runs as a background system with a dashboard modal for monitoring.

pub mod api;
mod migrations;
pub mod repository;

pub use api::*;
pub use repository::{EnvSync, IndexerRepository, RepositoryError, SyncLogEntry, SyncStatus};

// System implementation will be added in Phase 5.
