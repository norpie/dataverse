//! Migration CRUD operations.

use async_sqlite::Client;
use chrono::Utc;
use rusqlite::params;

use super::super::types::*;
use super::RepositoryError;
use super::helpers::parse_datetime;

/// Input for creating a new migration.
pub struct NewMigration {
    pub name: String,
    pub description: Option<String>,
    pub source_environment_id: i64,
    pub target_environment_id: i64,
}

/// Input for updating a migration.
pub struct UpdateMigration {
    pub name: Option<String>,
    pub description: Option<String>,
}

impl super::MigrationRepository {
    /// List all migrations with summary information.
    pub async fn list_migrations(&self) -> Result<Vec<MigrationSummary>, RepositoryError> {
        self.client
            .conn(|conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, name, description, source_environment_id, target_environment_id, created_at, updated_at
                     FROM migrations
                     ORDER BY updated_at DESC",
                )?;
                let rows = stmt.query_map([], |row| {
                    Ok(MigrationSummary {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        description: row.get(2)?,
                        source_environment_id: row.get(3)?,
                        target_environment_id: row.get(4)?,
                        created_at: parse_datetime(&row.get::<_, String>(5)?)?,
                        updated_at: parse_datetime(&row.get::<_, String>(6)?)?,
                    })
                })?;
                rows.collect::<Result<Vec<_>, _>>()
            })
            .await
            .map_err(RepositoryError::Database)
    }

    /// Get a migration by ID (without phases).
    pub async fn get_migration(&self, id: i64) -> Result<Migration, RepositoryError> {
        self.client
            .conn(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, name, description, source_environment_id, target_environment_id, created_at, updated_at
                     FROM migrations
                     WHERE id = ?1",
                )?;
                stmt.query_row([id], |row| {
                    Ok(Migration {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        description: row.get(2)?,
                        source_environment_id: row.get(3)?,
                        target_environment_id: row.get(4)?,
                        created_at: parse_datetime(&row.get::<_, String>(5)?)?,
                        updated_at: parse_datetime(&row.get::<_, String>(6)?)?,
                    })
                })
            })
            .await
            .map_err(|e| match e {
                async_sqlite::Error::Rusqlite(rusqlite::Error::QueryReturnedNoRows) => {
                    RepositoryError::NotFound("Migration", id)
                }
                _ => RepositoryError::Database(e),
            })
    }

    /// Create a new migration.
    pub async fn create_migration(&self, migration: NewMigration) -> Result<i64, RepositoryError> {
        let now = Utc::now().to_rfc3339();
        self.client
            .conn(move |conn| {
                conn.execute(
                    "INSERT INTO migrations (name, description, source_environment_id, target_environment_id, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        migration.name,
                        migration.description,
                        migration.source_environment_id,
                        migration.target_environment_id,
                        now,
                        now,
                    ],
                )?;
                Ok(conn.last_insert_rowid())
            })
            .await
            .map_err(RepositoryError::Database)
    }

    /// Update a migration.
    pub async fn update_migration(
        &self,
        id: i64,
        update: UpdateMigration,
    ) -> Result<(), RepositoryError> {
        let now = Utc::now().to_rfc3339();
        let client = self.client.clone();

        client
            .conn(move |conn| {
                let mut updates = Vec::new();
                let mut param_vals: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

                if let Some(name) = update.name {
                    updates.push("name = ?");
                    param_vals.push(Box::new(name));
                }
                if update.description.is_some() {
                    updates.push("description = ?");
                    param_vals.push(Box::new(update.description));
                }

                if updates.is_empty() {
                    return Ok(0);
                }

                updates.push("updated_at = ?");
                param_vals.push(Box::new(now));
                param_vals.push(Box::new(id));

                let sql = format!("UPDATE migrations SET {} WHERE id = ?", updates.join(", "));
                let param_refs: Vec<&dyn rusqlite::ToSql> =
                    param_vals.iter().map(|p| p.as_ref()).collect();
                conn.execute(&sql, param_refs.as_slice())
            })
            .await
            .map_err(RepositoryError::Database)
            .and_then(|affected| {
                if affected == 0 {
                    Err(RepositoryError::NotFound("Migration", id))
                } else {
                    Ok(())
                }
            })
    }

    /// Delete a migration (cascades to all related entities).
    pub async fn delete_migration(&self, id: i64) -> Result<(), RepositoryError> {
        self.client
            .conn(move |conn| conn.execute("DELETE FROM migrations WHERE id = ?1", [id]))
            .await
            .map_err(RepositoryError::Database)
            .and_then(|affected| {
                if affected == 0 {
                    Err(RepositoryError::NotFound("Migration", id))
                } else {
                    Ok(())
                }
            })
    }
}
