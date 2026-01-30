//! Indexer database repository.

use async_sqlite::Client;
use async_sqlite::ClientBuilder;
use async_sqlite::JournalMode;
use chrono::{DateTime, TimeZone, Utc};
use thiserror::Error;

use super::migrations;

/// Repository error type.
#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error("database error: {0}")]
    Database(#[from] async_sqlite::Error),

    #[error("migration error: {0}")]
    Migration(#[from] crate::migrations::MigrationError),

    #[error("serialization error: {0}")]
    Serialization(#[from] bincode::error::EncodeError),

    #[error("deserialization error: {0}")]
    Deserialization(#[from] bincode::error::DecodeError),
}

/// Sync status for an environment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncStatus {
    Idle,
    Syncing,
    Paused,
    Error,
}

impl SyncStatus {
    pub fn from_str(s: &str) -> Self {
        match s {
            "idle" => Self::Idle,
            "syncing" => Self::Syncing,
            "paused" => Self::Paused,
            "error" => Self::Error,
            _ => Self::Idle,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Syncing => "syncing",
            Self::Paused => "paused",
            Self::Error => "error",
        }
    }
}

/// Environment sync state.
#[derive(Debug, Clone)]
pub struct EnvSync {
    pub env_id: i64,
    pub status: SyncStatus,
    pub last_sync_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub entities_count: i64,
    pub global_optionsets_count: i64,
    pub total_attributes_count: i64,
}

/// Sync log entry.
#[derive(Debug, Clone)]
pub struct SyncLogEntry {
    pub id: i64,
    pub env_id: i64,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub status: String,
    pub error: Option<String>,
    pub entities_fetched: i64,
    pub optionsets_fetched: i64,
}

/// Repository for indexer database operations.
#[derive(Clone)]
pub struct IndexerRepository {
    client: Client,
}

impl IndexerRepository {
    /// Open or create the indexer database at the given path.
    pub async fn new(path: impl AsRef<std::path::Path>) -> Result<Self, RepositoryError> {
        let client = ClientBuilder::new()
            .path(path)
            .journal_mode(JournalMode::Wal)
            .open()
            .await?;

        // Apply migrations
        let migrations = migrations::load()?;
        crate::migrations::run(&client, &migrations).await?;

        Ok(Self { client })
    }

    /// Get sync state for an environment.
    pub async fn get_env_sync(&self, env_id: i64) -> Result<Option<EnvSync>, RepositoryError> {
        self.client
            .conn(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT env_id, status, last_sync_at, last_error, entities_count, \
                     global_optionsets_count, total_attributes_count \
                     FROM environment_sync WHERE env_id = ?",
                )?;

                let result = stmt.query_row([env_id], |row| {
                    let status_str: String = row.get(1)?;
                    let last_sync_at: Option<i64> = row.get(2)?;
                    let last_sync_at = last_sync_at.and_then(|ts| Utc.timestamp_opt(ts, 0).single());

                    Ok(EnvSync {
                        env_id: row.get(0)?,
                        status: SyncStatus::from_str(&status_str),
                        last_sync_at,
                        last_error: row.get(3)?,
                        entities_count: row.get(4)?,
                        global_optionsets_count: row.get(5)?,
                        total_attributes_count: row.get(6)?,
                    })
                });

                match result {
                    Ok(env_sync) => Ok(Some(env_sync)),
                    Err(async_sqlite::rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                    Err(e) => Err(e),
                }
            })
            .await
            .map_err(RepositoryError::Database)
    }

    /// Get all environment sync states.
    pub async fn get_all_env_syncs(&self) -> Result<Vec<EnvSync>, RepositoryError> {
        self.client
            .conn(|conn| {
                let mut stmt = conn.prepare(
                    "SELECT env_id, status, last_sync_at, last_error, entities_count, \
                     global_optionsets_count, total_attributes_count \
                     FROM environment_sync ORDER BY env_id",
                )?;

                let rows = stmt.query_map([], |row| {
                    let status_str: String = row.get(1)?;
                    let last_sync_at: Option<i64> = row.get(2)?;
                    let last_sync_at = last_sync_at.and_then(|ts| Utc.timestamp_opt(ts, 0).single());

                    Ok(EnvSync {
                        env_id: row.get(0)?,
                        status: SyncStatus::from_str(&status_str),
                        last_sync_at,
                        last_error: row.get(3)?,
                        entities_count: row.get(4)?,
                        global_optionsets_count: row.get(5)?,
                        total_attributes_count: row.get(6)?,
                    })
                })?;

                rows.collect()
            })
            .await
            .map_err(RepositoryError::Database)
    }

    /// Upsert environment sync state.
    pub async fn upsert_env_sync(
        &self,
        env_id: i64,
        status: SyncStatus,
        last_sync_at: Option<DateTime<Utc>>,
        last_error: Option<String>,
        entities_count: i64,
        global_optionsets_count: i64,
        total_attributes_count: i64,
    ) -> Result<(), RepositoryError> {
        let status_str = status.as_str().to_string();
        let last_sync_ts = last_sync_at.map(|dt| dt.timestamp());

        self.client
            .conn(move |conn| {
                conn.execute(
                    "INSERT INTO environment_sync \
                     (env_id, status, last_sync_at, last_error, entities_count, \
                      global_optionsets_count, total_attributes_count) \
                     VALUES (?, ?, ?, ?, ?, ?, ?) \
                     ON CONFLICT(env_id) DO UPDATE SET \
                     status = excluded.status, \
                     last_sync_at = excluded.last_sync_at, \
                     last_error = excluded.last_error, \
                     entities_count = excluded.entities_count, \
                     global_optionsets_count = excluded.global_optionsets_count, \
                     total_attributes_count = excluded.total_attributes_count",
                    (
                        env_id,
                        status_str,
                        last_sync_ts,
                        last_error,
                        entities_count,
                        global_optionsets_count,
                        total_attributes_count,
                    ),
                )
            })
            .await
            .map_err(RepositoryError::Database)?;

        Ok(())
    }

    /// Clear sync state for an environment (reset to defaults).
    pub async fn clear_env_sync(&self, env_id: i64) -> Result<(), RepositoryError> {
        self.client
            .conn(move |conn| {
                conn.execute("DELETE FROM environment_sync WHERE env_id = ?", [env_id])
            })
            .await
            .map_err(RepositoryError::Database)?;

        Ok(())
    }

    /// Add a sync log entry.
    pub async fn add_sync_log(
        &self,
        env_id: i64,
        started_at: DateTime<Utc>,
        completed_at: Option<DateTime<Utc>>,
        status: String,
        error: Option<String>,
        entities_fetched: i64,
        optionsets_fetched: i64,
    ) -> Result<i64, RepositoryError> {
        let started_ts = started_at.timestamp();
        let completed_ts = completed_at.map(|dt| dt.timestamp());

        let id = self
            .client
            .conn(move |conn| {
                conn.execute(
                    "INSERT INTO sync_log \
                     (env_id, started_at, completed_at, status, error, entities_fetched, optionsets_fetched) \
                     VALUES (?, ?, ?, ?, ?, ?, ?)",
                    (
                        env_id,
                        started_ts,
                        completed_ts,
                        status,
                        error,
                        entities_fetched,
                        optionsets_fetched,
                    ),
                )?;
                Ok(conn.last_insert_rowid())
            })
            .await
            .map_err(RepositoryError::Database)?;

        Ok(id)
    }

    /// Get recent sync logs for a specific environment.
    pub async fn get_sync_logs(
        &self,
        env_id: i64,
        limit: usize,
    ) -> Result<Vec<SyncLogEntry>, RepositoryError> {
        self.client
            .conn(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, env_id, started_at, completed_at, status, error, \
                     entities_fetched, optionsets_fetched \
                     FROM sync_log WHERE env_id = ? \
                     ORDER BY started_at DESC LIMIT ?",
                )?;

                let rows = stmt.query_map([env_id, limit as i64], |row| {
                    let started_ts: i64 = row.get(2)?;
                    let completed_ts: Option<i64> = row.get(3)?;

                    Ok(SyncLogEntry {
                        id: row.get(0)?,
                        env_id: row.get(1)?,
                        started_at: Utc.timestamp_opt(started_ts, 0).single().unwrap(),
                        completed_at: completed_ts.and_then(|ts| Utc.timestamp_opt(ts, 0).single()),
                        status: row.get(4)?,
                        error: row.get(5)?,
                        entities_fetched: row.get(6)?,
                        optionsets_fetched: row.get(7)?,
                    })
                })?;

                rows.collect()
            })
            .await
            .map_err(RepositoryError::Database)
    }

    /// Get recent sync logs across all environments.
    pub async fn get_all_sync_logs(&self, limit: usize) -> Result<Vec<SyncLogEntry>, RepositoryError> {
        self.client
            .conn(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, env_id, started_at, completed_at, status, error, \
                     entities_fetched, optionsets_fetched \
                     FROM sync_log \
                     ORDER BY started_at DESC LIMIT ?",
                )?;

                let rows = stmt.query_map([limit as i64], |row| {
                    let started_ts: i64 = row.get(2)?;
                    let completed_ts: Option<i64> = row.get(3)?;

                    Ok(SyncLogEntry {
                        id: row.get(0)?,
                        env_id: row.get(1)?,
                        started_at: Utc.timestamp_opt(started_ts, 0).single().unwrap(),
                        completed_at: completed_ts.and_then(|ts| Utc.timestamp_opt(ts, 0).single()),
                        status: row.get(4)?,
                        error: row.get(5)?,
                        entities_fetched: row.get(6)?,
                        optionsets_fetched: row.get(7)?,
                    })
                })?;

                rows.collect()
            })
            .await
            .map_err(RepositoryError::Database)
    }

    // =========================================================================
    // Settings Operations
    // =========================================================================

}
