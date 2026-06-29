//! Query Builder SQLite repository for saving/loading queries.

use async_sqlite::Client;
use async_sqlite::ClientBuilder;
use chrono::DateTime;
use chrono::Utc;
use std::path::Path;
use thiserror::Error;

use super::data::QueryData;

/// Errors from query repository operations.
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
    #[error("Query not found: {0}")]
    NotFound(i64),
}

/// Summary of a saved query (for listing).
#[derive(Debug, Clone)]
pub struct SavedQuerySummary {
    pub id: i64,
    pub name: String,
    pub entity: Option<String>,
    pub updated_at: DateTime<Utc>,
}

/// A fully loaded saved query.
#[derive(Debug, Clone)]
pub struct SavedQuery {
    pub id: i64,
    pub name: String,
    pub data: QueryData,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Query repository for SQLite persistence.
#[derive(Clone)]
pub struct QueryRepository {
    client: Client,
}

impl QueryRepository {
    /// Create a new repository, initializing the database schema.
    pub async fn new(path: &Path) -> Result<Self, RepositoryError> {
        let client = ClientBuilder::new().path(path).open().await?;

        // Run migrations
        let migrations = super::migrations::load()?;
        crate::migrations::run(&client, &migrations).await?;

        Ok(Self { client })
    }

    /// Save a query. If `id` is Some, updates the existing entry; otherwise inserts a new one.
    /// Returns the ID of the saved query.
    pub async fn save(
        &self,
        id: Option<i64>,
        name: String,
        data: &QueryData,
    ) -> Result<i64, RepositoryError> {
        let data_bytes = bincode::serde::encode_to_vec(data, bincode::config::standard())?;
        let entity = data.entity.as_ref().map(|entity| entity.name().to_string());
        let now = Utc::now().to_rfc3339();

        match id {
            Some(existing_id) => {
                self.client
                    .conn(move |conn| {
                        conn.execute(
                            "UPDATE saved_queries SET name = ?1, entity = ?2, data = ?3, updated_at = ?4
                             WHERE id = ?5",
                            rusqlite::params![name, entity, data_bytes, now, existing_id],
                        )?;
                        Ok(existing_id)
                    })
                    .await
                    .map_err(RepositoryError::Database)
            }
            None => {
                self.client
                    .conn(move |conn| {
                        conn.execute(
                            "INSERT INTO saved_queries (name, entity, data, created_at, updated_at)
                             VALUES (?1, ?2, ?3, ?4, ?5)",
                            rusqlite::params![name, entity, data_bytes, now, now],
                        )?;
                        Ok(conn.last_insert_rowid())
                    })
                    .await
                    .map_err(RepositoryError::Database)
            }
        }
    }

    /// Load a saved query by ID.
    pub async fn load(&self, id: i64) -> Result<SavedQuery, RepositoryError> {
        self.client
            .conn(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, name, data, created_at, updated_at
                     FROM saved_queries WHERE id = ?1",
                )?;
                let query = stmt.query_row([id], |row| {
                    let id: i64 = row.get(0)?;
                    let name: String = row.get(1)?;
                    let data_bytes: Vec<u8> = row.get(2)?;
                    let created_at: String = row.get(3)?;
                    let updated_at: String = row.get(4)?;
                    Ok((id, name, data_bytes, created_at, updated_at))
                })?;
                Ok(query)
            })
            .await
            .map_err(|e| match e {
                async_sqlite::Error::Rusqlite(rusqlite::Error::QueryReturnedNoRows) => {
                    RepositoryError::NotFound(id)
                }
                _ => RepositoryError::Database(e),
            })
            .and_then(|(id, name, data_bytes, created_at, updated_at)| {
                let (data, _): (QueryData, _) =
                    bincode::serde::decode_from_slice(&data_bytes, bincode::config::standard())?;
                let created_at = DateTime::parse_from_rfc3339(&created_at)
                    .unwrap_or_default()
                    .with_timezone(&Utc);
                let updated_at = DateTime::parse_from_rfc3339(&updated_at)
                    .unwrap_or_default()
                    .with_timezone(&Utc);
                Ok(SavedQuery {
                    id,
                    name,
                    data,
                    created_at,
                    updated_at,
                })
            })
    }

    /// List all saved queries (summaries only, sorted by most recently updated).
    pub async fn list(&self) -> Result<Vec<SavedQuerySummary>, RepositoryError> {
        self.client
            .conn(|conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, name, entity, updated_at
                     FROM saved_queries
                     ORDER BY updated_at DESC",
                )?;
                let rows = stmt
                    .query_map([], |row| {
                        let id: i64 = row.get(0)?;
                        let name: String = row.get(1)?;
                        let entity: Option<String> = row.get(2)?;
                        let updated_at: String = row.get(3)?;
                        Ok((id, name, entity, updated_at))
                    })?
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(rows)
            })
            .await
            .map_err(RepositoryError::Database)
            .map(|rows| {
                rows.into_iter()
                    .map(|(id, name, entity, updated_at)| {
                        let updated_at = DateTime::parse_from_rfc3339(&updated_at)
                            .unwrap_or_default()
                            .with_timezone(&Utc);
                        SavedQuerySummary {
                            id,
                            name,
                            entity,
                            updated_at,
                        }
                    })
                    .collect()
            })
    }

    /// Delete a saved query by ID.
    pub async fn delete(&self, id: i64) -> Result<(), RepositoryError> {
        self.client
            .conn(move |conn| {
                conn.execute("DELETE FROM saved_queries WHERE id = ?1", [id])?;
                Ok(())
            })
            .await
            .map_err(RepositoryError::Database)
    }
}
