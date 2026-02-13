//! Queue SQLite repository.

use async_sqlite::Client;
use async_sqlite::ClientBuilder;
use chrono::DateTime;
use chrono::Utc;
use std::path::Path;
use thiserror::Error;

use super::types::ExecutionRecord;
use super::types::ExecutionStatus;
use super::types::ItemStatus;
use super::types::OperationResultRecord;
use super::types::QueueItem;
use super::types::QueueItemId;
use super::types::QueuePayload;

/// Errors from queue repository operations.
#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error("Database error: {0}")]
    Database(#[from] async_sqlite::Error),
    #[error("Migration error: {0}")]
    Migration(#[from] crate::migrations::MigrationError),
    #[error("Serialization error: {0}")]
    Serialization(#[from] bincode::error::EncodeError),
    #[error("Deserialization error: {0}")]
    Deserialization(#[from] bincode::error::DecodeError),
    #[error("Item not found: {0}")]
    NotFound(QueueItemId),
}

/// Queue repository for SQLite persistence.
#[derive(Clone)]
pub struct QueueRepository {
    client: Client,
}

impl QueueRepository {
    /// Create a new repository, initializing the database schema.
    pub async fn new(path: &Path) -> Result<Self, RepositoryError> {
        let client = ClientBuilder::new().path(path).open().await?;

        // Run migrations
        let migrations = super::migrations::load()?;
        crate::migrations::run(&client, &migrations).await?;

        Ok(Self { client })
    }

    // =========================================================================
    // Queue Item Operations
    // =========================================================================

    /// Insert a new queue item, returning its ID.
    pub async fn insert(&self, item: NewQueueItem) -> Result<QueueItemId, RepositoryError> {
        let payload_bytes =
            bincode::serde::encode_to_vec(&item.payload, bincode::config::standard())?;
        let status = status_to_str(item.status);
        let created_at = item.created_at.to_rfc3339();

        let id = self
            .client
            .conn(move |conn| {
                conn.execute(
                    "INSERT INTO queue_items (priority, status, payload, env_id, account_id, source, description, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    rusqlite::params![
                        item.priority,
                        status,
                        payload_bytes,
                        item.env_id,
                        item.account_id,
                        item.source,
                        item.description,
                        created_at,
                    ],
                )?;
                Ok(conn.last_insert_rowid())
            })
            .await?;

        Ok(id)
    }

    /// Get a queue item by ID.
    pub async fn get(&self, id: QueueItemId) -> Result<QueueItem, RepositoryError> {
        self.client
            .conn(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, priority, status, payload, env_id, account_id, source, description, created_at
                     FROM queue_items WHERE id = ?1",
                )?;
                let item = stmt.query_row([id], row_to_item)?;
                Ok(item)
            })
            .await
            .map_err(|e| match e {
                async_sqlite::Error::Rusqlite(rusqlite::Error::QueryReturnedNoRows) => {
                    RepositoryError::NotFound(id)
                }
                _ => RepositoryError::Database(e),
            })
    }

    /// Get the next ready item (highest priority, oldest first).
    pub async fn get_next_ready(&self) -> Result<Option<QueueItem>, RepositoryError> {
        self.client
            .conn(|conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, priority, status, payload, env_id, account_id, source, description, created_at
                     FROM queue_items
                     WHERE status = 'ready'
                     ORDER BY priority DESC, created_at ASC
                     LIMIT 1",
                )?;
                match stmt.query_row([], row_to_item) {
                    Ok(item) => Ok(Some(item)),
                    Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                    Err(e) => Err(e),
                }
            })
            .await
            .map_err(RepositoryError::Database)
    }

    /// Update item status.
    pub async fn update_status(
        &self,
        id: QueueItemId,
        status: ItemStatus,
    ) -> Result<(), RepositoryError> {
        let status_str = status_to_str(status);
        let affected = self
            .client
            .conn(move |conn| {
                conn.execute(
                    "UPDATE queue_items SET status = ?1 WHERE id = ?2",
                    rusqlite::params![status_str, id],
                )
            })
            .await?;

        if affected == 0 {
            return Err(RepositoryError::NotFound(id));
        }
        Ok(())
    }

    /// Delete a queue item.
    pub async fn delete(&self, id: QueueItemId) -> Result<(), RepositoryError> {
        let affected = self
            .client
            .conn(move |conn| {
                conn.execute(
                    "DELETE FROM queue_items WHERE id = ?1",
                    rusqlite::params![id],
                )
            })
            .await?;

        if affected == 0 {
            return Err(RepositoryError::NotFound(id));
        }
        Ok(())
    }

    /// Delete multiple queue items by ID (skips running items).
    /// Returns the number of items actually deleted.
    pub async fn delete_many(&self, ids: Vec<QueueItemId>) -> Result<usize, RepositoryError> {
        if ids.is_empty() {
            return Ok(0);
        }

        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");

        let affected = self
            .client
            .conn(move |conn| {
                let sql = format!(
                    "DELETE FROM queue_items WHERE id IN ({}) AND status != 'running'",
                    placeholders
                );
                let params: Vec<Box<dyn rusqlite::types::ToSql>> =
                    ids.into_iter().map(|id| Box::new(id) as _).collect();
                let params_refs: Vec<&dyn rusqlite::types::ToSql> =
                    params.iter().map(|p| p.as_ref()).collect();
                conn.execute(&sql, params_refs.as_slice())
            })
            .await?;

        Ok(affected)
    }

    /// Delete all non-running queue items with a specific source.
    /// Returns the number of items deleted.
    pub async fn delete_by_source(&self, source: String) -> Result<usize, RepositoryError> {
        let affected = self
            .client
            .conn(move |conn| {
                conn.execute(
                    "DELETE FROM queue_items WHERE source = ?1 AND status != 'running'",
                    rusqlite::params![source],
                )
            })
            .await?;

        Ok(affected)
    }

    /// List items with optional filtering.
    pub async fn list(&self, filter: ListFilter) -> Result<Vec<QueueItem>, RepositoryError> {
        let statuses = filter.statuses.map(|s| {
            s.iter()
                .map(|st| format!("'{}'", status_to_str(*st)))
                .collect::<Vec<_>>()
                .join(",")
        });
        let sources = filter.sources.clone();
        let search = filter.search.clone();

        self.client
            .conn(move |conn| {
                let mut sql = String::from(
                    "SELECT id, priority, status, payload, env_id, account_id, source, description, created_at
                     FROM queue_items WHERE 1=1",
                );

                if let Some(ref status_list) = statuses {
                    sql.push_str(&format!(" AND status IN ({})", status_list));
                }
                if let Some(ref source_list) = sources {
                    let sources_str = source_list
                        .iter()
                        .map(|s| format!("'{}'", s.replace('\'', "''")))
                        .collect::<Vec<_>>()
                        .join(",");
                    sql.push_str(&format!(" AND source IN ({})", sources_str));
                }
                if let Some(ref search_text) = search {
                    let escaped = search_text.replace('\'', "''");
                    sql.push_str(&format!(" AND description LIKE '%{}%'", escaped));
                }

                sql.push_str(" ORDER BY priority DESC, created_at ASC");

                let mut stmt = conn.prepare(&sql)?;
                let items = stmt
                    .query_map([], row_to_item)?
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(items)
            })
            .await
            .map_err(RepositoryError::Database)
    }

    /// Count items by status.
    pub async fn count_by_status(&self) -> Result<StatusCounts, RepositoryError> {
        self.client
            .conn(|conn| {
                let mut stmt =
                    conn.prepare("SELECT status, COUNT(*) FROM queue_items GROUP BY status")?;
                let mut counts = StatusCounts::default();
                let rows = stmt.query_map([], |row| {
                    let status: String = row.get(0)?;
                    let count: i64 = row.get(1)?;
                    Ok((status, count as usize))
                })?;
                for row in rows {
                    let (status, count) = row?;
                    match status.as_str() {
                        "blocked" => counts.blocked = count,
                        "ready" => counts.ready = count,
                        "paused" => counts.paused = count,
                        "running" => counts.running = count,
                        "interrupted" => counts.interrupted = count,
                        "done" => counts.done = count,
                        "failed" => counts.failed = count,
                        "partially_failed" => counts.partially_failed = count,
                        _ => {}
                    }
                }
                Ok(counts)
            })
            .await
            .map_err(RepositoryError::Database)
    }

    /// Transition all Running items to Interrupted (for crash recovery).
    pub async fn mark_running_as_interrupted(&self) -> Result<usize, RepositoryError> {
        let affected = self
            .client
            .conn(|conn| {
                conn.execute(
                    "UPDATE queue_items SET status = 'interrupted' WHERE status = 'running'",
                    [],
                )
            })
            .await?;
        Ok(affected)
    }

    /// Transition items for an environment between Blocked and Ready.
    pub async fn update_environment_availability(
        &self,
        env_id: i64,
        available: bool,
    ) -> Result<usize, RepositoryError> {
        let (from_status, to_status) = if available {
            ("blocked", "ready")
        } else {
            ("ready", "blocked")
        };

        let affected = self
            .client
            .conn(move |conn| {
                conn.execute(
                    "UPDATE queue_items SET status = ?1 WHERE env_id = ?2 AND status = ?3",
                    rusqlite::params![to_status, env_id, from_status],
                )
            })
            .await?;
        Ok(affected)
    }

    /// Get all unique sources.
    pub async fn get_sources(&self) -> Result<Vec<String>, RepositoryError> {
        self.client
            .conn(|conn| {
                let mut stmt =
                    conn.prepare("SELECT DISTINCT source FROM queue_items ORDER BY source")?;
                let sources = stmt
                    .query_map([], |row| row.get(0))?
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(sources)
            })
            .await
            .map_err(RepositoryError::Database)
    }

    /// Clear all completed items (Done, Failed, PartiallyFailed).
    pub async fn clear_completed(&self) -> Result<usize, RepositoryError> {
        let affected = self
            .client
            .conn(|conn| {
                conn.execute(
                    "DELETE FROM queue_items WHERE status IN ('done', 'failed', 'partially_failed')",
                    [],
                )
            })
            .await?;
        Ok(affected)
    }

    /// Clear all non-running items.
    pub async fn clear_all(&self) -> Result<usize, RepositoryError> {
        let affected = self
            .client
            .conn(|conn| conn.execute("DELETE FROM queue_items WHERE status != 'running'", []))
            .await?;
        Ok(affected)
    }

    /// Update item fields (priority, description, source, env_id).
    pub async fn update_item(
        &self,
        id: QueueItemId,
        update: UpdateItem,
    ) -> Result<(), RepositoryError> {
        let affected = self
            .client
            .conn(move |conn| {
                conn.execute(
                    "UPDATE queue_items SET priority = ?1, description = ?2, source = ?3, env_id = ?4
                     WHERE id = ?5",
                    rusqlite::params![
                        update.priority,
                        update.description,
                        update.source,
                        update.env_id,
                        id,
                    ],
                )
            })
            .await?;

        if affected == 0 {
            return Err(RepositoryError::NotFound(id));
        }
        Ok(())
    }

    /// Retry an item by cloning its payload into a new Ready item.
    /// Returns the new item's ID.
    pub async fn retry_item(&self, id: QueueItemId) -> Result<QueueItemId, RepositoryError> {
        let original = self.get(id).await?;
        let new_item = NewQueueItem {
            priority: original.priority,
            status: ItemStatus::Ready,
            payload: original.payload,
            env_id: original.env_id,
            account_id: original.account_id,
            source: original.source,
            description: original.description,
            created_at: Utc::now(),
        };
        self.insert(new_item).await
    }

    // =========================================================================
    // Settings Operations
    // =========================================================================

    // =========================================================================
    // Execution History Operations
    // =========================================================================

    /// Insert an execution record.
    pub async fn insert_execution(
        &self,
        record: NewExecutionRecord,
    ) -> Result<i64, RepositoryError> {
        let status = execution_status_to_str(record.status);
        let started_at = record.started_at.to_rfc3339();
        let completed_at = record.completed_at.to_rfc3339();

        let id = self
            .client
            .conn(move |conn| {
                conn.execute(
                    "INSERT INTO execution_history (item_id, started_at, completed_at, duration_ms, status, error, success_count, failure_count)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    rusqlite::params![
                        record.item_id,
                        started_at,
                        completed_at,
                        record.duration_ms,
                        status,
                        record.error,
                        record.success_count,
                        record.failure_count,
                    ],
                )?;
                Ok(conn.last_insert_rowid())
            })
            .await?;

        Ok(id)
    }

    /// Get execution history for an item.
    pub async fn get_executions(
        &self,
        item_id: QueueItemId,
    ) -> Result<Vec<ExecutionRecord>, RepositoryError> {
        self.client
            .conn(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, item_id, started_at, completed_at, duration_ms, status, error, success_count, failure_count
                     FROM execution_history
                     WHERE item_id = ?1
                     ORDER BY started_at DESC",
                )?;
                let records = stmt
                    .query_map([item_id], row_to_execution)?
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(records)
            })
            .await
            .map_err(RepositoryError::Database)
    }

    // =========================================================================
    // Batch Operation Results
    // =========================================================================

    /// Insert batch operation results for an execution.
    pub async fn insert_operation_results(
        &self,
        results: Vec<NewOperationResult>,
    ) -> Result<(), RepositoryError> {
        if results.is_empty() {
            return Ok(());
        }

        self.client
            .conn(move |conn| {
                let mut stmt = conn.prepare(
                    "INSERT INTO batch_operation_results 
                     (execution_id, op_index, content_id, success, operation_type, result_data, error_status, error_code, error_message)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                )?;

                for r in results {
                    stmt.execute(rusqlite::params![
                        r.execution_id,
                        r.op_index,
                        r.content_id,
                        r.success as i32,
                        r.operation_type,
                        r.result_data,
                        r.error_status,
                        r.error_code,
                        r.error_message,
                    ])?;
                }

                Ok(())
            })
            .await
            .map_err(RepositoryError::Database)
    }

    /// Get operation results for an execution.
    pub async fn get_operation_results(
        &self,
        execution_id: i64,
    ) -> Result<Vec<OperationResultRecord>, RepositoryError> {
        self.client
            .conn(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT execution_id, op_index, content_id, success, operation_type, result_data, error_status, error_code, error_message
                     FROM batch_operation_results
                     WHERE execution_id = ?1
                     ORDER BY op_index",
                )?;
                let records = stmt
                    .query_map([execution_id], row_to_operation_result)?
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(records)
            })
            .await
            .map_err(RepositoryError::Database)
    }

    /// Get operation results by content_id across all executions for a queue item.
    pub async fn get_operation_results_by_content_id(
        &self,
        item_id: QueueItemId,
        content_id: &str,
    ) -> Result<Vec<OperationResultRecord>, RepositoryError> {
        let content_id = content_id.to_string();
        self.client
            .conn(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT r.execution_id, r.op_index, r.content_id, r.success, r.operation_type, r.result_data, r.error_status, r.error_code, r.error_message
                     FROM batch_operation_results r
                     JOIN execution_history e ON r.execution_id = e.id
                     WHERE e.item_id = ?1 AND r.content_id = ?2
                     ORDER BY e.started_at DESC, r.op_index",
                )?;
                let records = stmt
                    .query_map(rusqlite::params![item_id, content_id], row_to_operation_result)?
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(records)
            })
            .await
            .map_err(RepositoryError::Database)
    }
}

// =============================================================================
// Helper Types
// =============================================================================

/// Data for creating a new queue item.
pub struct NewQueueItem {
    pub priority: i32,
    pub status: ItemStatus,
    pub payload: QueuePayload,
    pub env_id: i64,
    pub account_id: i64,
    pub source: String,
    pub description: String,
    pub created_at: DateTime<Utc>,
}

/// Data for updating an existing queue item.
#[derive(Clone)]
pub struct UpdateItem {
    pub priority: i32,
    pub description: String,
    pub source: String,
    pub env_id: i64,
}

/// Data for creating a new execution record.
pub struct NewExecutionRecord {
    pub item_id: QueueItemId,
    pub started_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
    pub duration_ms: i64,
    pub status: ExecutionStatus,
    pub error: Option<String>,
    pub success_count: i32,
    pub failure_count: i32,
}

/// Data for creating a new operation result.
pub struct NewOperationResult {
    pub execution_id: i64,
    pub op_index: i32,
    pub content_id: Option<String>,
    pub success: bool,
    pub operation_type: Option<String>,
    pub result_data: Option<String>,
    pub error_status: Option<i32>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}

/// Filter options for listing queue items.
#[derive(Default)]
pub struct ListFilter {
    pub statuses: Option<Vec<ItemStatus>>,
    pub sources: Option<Vec<String>>,
    pub search: Option<String>,
}

/// Counts of items by status.
#[derive(Debug, Default, Clone)]
pub struct StatusCounts {
    pub blocked: usize,
    pub ready: usize,
    pub paused: usize,
    pub running: usize,
    pub interrupted: usize,
    pub done: usize,
    pub failed: usize,
    pub partially_failed: usize,
}

impl StatusCounts {
    /// Total number of items.
    pub fn total(&self) -> usize {
        self.blocked
            + self.ready
            + self.paused
            + self.running
            + self.interrupted
            + self.done
            + self.failed
            + self.partially_failed
    }

    /// Number of pending items (ready + paused + blocked).
    pub fn pending(&self) -> usize {
        self.ready + self.paused + self.blocked
    }
}

// =============================================================================
// Conversion Helpers
// =============================================================================

fn status_to_str(status: ItemStatus) -> &'static str {
    match status {
        ItemStatus::Blocked => "blocked",
        ItemStatus::Ready => "ready",
        ItemStatus::Paused => "paused",
        ItemStatus::Running => "running",
        ItemStatus::Interrupted => "interrupted",
        ItemStatus::Done => "done",
        ItemStatus::Failed => "failed",
        ItemStatus::PartiallyFailed => "partially_failed",
    }
}

fn str_to_status(s: &str) -> ItemStatus {
    match s {
        "blocked" => ItemStatus::Blocked,
        "ready" => ItemStatus::Ready,
        "paused" => ItemStatus::Paused,
        "running" => ItemStatus::Running,
        "interrupted" => ItemStatus::Interrupted,
        "done" => ItemStatus::Done,
        "failed" => ItemStatus::Failed,
        "partially_failed" => ItemStatus::PartiallyFailed,
        _ => ItemStatus::Ready, // fallback
    }
}

fn execution_status_to_str(status: ExecutionStatus) -> &'static str {
    match status {
        ExecutionStatus::Success => "success",
        ExecutionStatus::Failed => "failed",
        ExecutionStatus::PartialSuccess => "partial_success",
    }
}

fn str_to_execution_status(s: &str) -> ExecutionStatus {
    match s {
        "success" => ExecutionStatus::Success,
        "failed" => ExecutionStatus::Failed,
        "partial_success" => ExecutionStatus::PartialSuccess,
        _ => ExecutionStatus::Failed, // fallback
    }
}

fn row_to_item(row: &rusqlite::Row) -> rusqlite::Result<QueueItem> {
    let payload_bytes: Vec<u8> = row.get(3)?;
    let (payload, _): (QueuePayload, _) = bincode::serde::decode_from_slice(
        &payload_bytes,
        bincode::config::standard(),
    )
    .map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(3, rusqlite::types::Type::Blob, Box::new(e))
    })?;

    let status_str: String = row.get(2)?;
    let created_at_str: String = row.get(8)?;
    let created_at = DateTime::parse_from_rfc3339(&created_at_str)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(8, rusqlite::types::Type::Text, Box::new(e))
        })?;

    Ok(QueueItem {
        id: row.get(0)?,
        priority: row.get(1)?,
        status: str_to_status(&status_str),
        payload,
        env_id: row.get(4)?,
        account_id: row.get(5)?,
        source: row.get(6)?,
        description: row.get(7)?,
        created_at,
    })
}

fn row_to_execution(row: &rusqlite::Row) -> rusqlite::Result<ExecutionRecord> {
    let status_str: String = row.get(5)?;
    let started_at_str: String = row.get(2)?;
    let completed_at_str: String = row.get(3)?;

    let started_at = DateTime::parse_from_rfc3339(&started_at_str)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Text, Box::new(e))
        })?;
    let completed_at = DateTime::parse_from_rfc3339(&completed_at_str)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(3, rusqlite::types::Type::Text, Box::new(e))
        })?;

    Ok(ExecutionRecord {
        id: row.get(0)?,
        item_id: row.get(1)?,
        started_at,
        completed_at,
        duration_ms: row.get(4)?,
        status: str_to_execution_status(&status_str),
        error: row.get(6)?,
        success_count: row.get(7)?,
        failure_count: row.get(8)?,
    })
}

fn row_to_operation_result(row: &rusqlite::Row) -> rusqlite::Result<OperationResultRecord> {
    let success: i32 = row.get(3)?;
    Ok(OperationResultRecord {
        execution_id: row.get(0)?,
        op_index: row.get(1)?,
        content_id: row.get(2)?,
        success: success != 0,
        operation_type: row.get(4)?,
        result_data: row.get(5)?,
        error_status: row.get(6)?,
        error_code: row.get(7)?,
        error_message: row.get(8)?,
    })
}
