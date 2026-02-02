//! Phase run CRUD operations.

use async_sqlite::Client;
use chrono::DateTime;
use chrono::Utc;
use rusqlite::params;

use super::super::types::*;
use super::RepositoryError;
use super::helpers::*;

/// Input for creating a new phase run.
pub struct NewPhaseRun {
    pub phase_id: i64,
    pub started_at: DateTime<Utc>,
}

impl super::MigrationRepository {
    /// Get all phase runs for a phase.
    pub async fn get_phase_runs(&self, phase_id: i64) -> Result<Vec<PhaseRun>, RepositoryError> {
        self.client
            .conn(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, phase_id, started_at, completed_at, status, queue_item_ids, error
                     FROM phase_runs
                     WHERE phase_id = ?1
                     ORDER BY started_at DESC",
                )?;
                let rows = stmt.query_map([phase_id], row_to_phase_run)?;
                rows.collect::<Result<Vec<_>, _>>()
            })
            .await
            .map_err(RepositoryError::Database)
    }

    /// Get a phase run by ID.
    pub async fn get_phase_run(&self, id: i64) -> Result<PhaseRun, RepositoryError> {
        self.client
            .conn(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, phase_id, started_at, completed_at, status, queue_item_ids, error
                     FROM phase_runs
                     WHERE id = ?1",
                )?;
                stmt.query_row([id], row_to_phase_run)
            })
            .await
            .map_err(|e| match e {
                async_sqlite::Error::Rusqlite(rusqlite::Error::QueryReturnedNoRows) => {
                    RepositoryError::NotFound("PhaseRun", id)
                }
                _ => RepositoryError::Database(e),
            })
    }

    /// Create a new phase run.
    pub async fn create_phase_run(&self, run: NewPhaseRun) -> Result<i64, RepositoryError> {
        let phase_id = run.phase_id;
        let started_at = run.started_at.to_rfc3339();

        self.client
            .conn(move |conn| {
                conn.execute(
                    "INSERT INTO phase_runs (phase_id, started_at, status)
                     VALUES (?1, ?2, 'running')",
                    params![phase_id, started_at],
                )?;
                Ok(conn.last_insert_rowid())
            })
            .await
            .map_err(RepositoryError::Database)
    }

    /// Update phase run status and completion data.
    pub async fn update_phase_run_status(
        &self,
        id: i64,
        status: PhaseRunStatus,
        queue_item_ids: Option<Vec<i64>>,
        error: Option<String>,
    ) -> Result<(), RepositoryError> {
        let status_str = status.as_str().to_string();
        let completed_at = if status != PhaseRunStatus::Running {
            Some(Utc::now().to_rfc3339())
        } else {
            None
        };
        let queue_item_ids_json = queue_item_ids
            .as_ref()
            .map(|ids| serialize_queue_item_ids(ids))
            .transpose()?;

        self.client
            .conn(move |conn| {
                conn.execute(
                    "UPDATE phase_runs SET status = ?1, completed_at = ?2, queue_item_ids = ?3, error = ?4
                     WHERE id = ?5",
                    params![status_str, completed_at, queue_item_ids_json, error, id],
                )
            })
            .await
            .map_err(RepositoryError::Database)
            .and_then(|affected| {
                if affected == 0 {
                    Err(RepositoryError::NotFound("PhaseRun", id))
                } else {
                    Ok(())
                }
            })
    }
}
