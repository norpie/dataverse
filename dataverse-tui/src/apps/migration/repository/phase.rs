//! Phase CRUD operations.

use async_sqlite::Client;
use chrono::Utc;
use rusqlite::params;

use super::super::types::*;
use super::RepositoryError;

/// Input for creating a new phase.
pub struct NewPhase {
    pub migration_id: i64,
    pub order: i32,
    pub name: String,
    pub mode: Mode,
    pub lua_script: Option<String>,
}

/// Input for updating a phase.
pub struct UpdatePhase {
    pub name: Option<String>,
    pub mode: Option<Mode>,
    pub lua_script: Option<String>,
}

impl super::MigrationRepository {
    /// Get all phases for a migration.
    pub async fn get_phases(&self, migration_id: i64) -> Result<Vec<Phase>, RepositoryError> {
        self.client
            .conn_mut(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, migration_id, \"order\", name, mode, lua_script
                     FROM phases
                     WHERE migration_id = ?1
                     ORDER BY \"order\" ASC",
                )?;
                let rows = stmt.query_map([migration_id], |row| {
                    let mode_str: String = row.get(4)?;
                    let mode =
                        Mode::from_str(&mode_str).ok_or_else(|| rusqlite::Error::InvalidQuery)?;

                    Ok(Phase {
                        id: row.get(0)?,
                        migration_id: row.get(1)?,
                        order: row.get(2)?,
                        name: row.get(3)?,
                        mode,
                        lua_script: row.get(5)?,
                    })
                })?;
                rows.collect::<Result<Vec<_>, _>>()
            })
            .await
            .map_err(RepositoryError::Database)
    }

    /// Get a phase by ID.
    pub async fn get_phase(&self, id: i64) -> Result<Phase, RepositoryError> {
        self.client
            .conn_mut(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, migration_id, \"order\", name, mode, lua_script
                     FROM phases
                     WHERE id = ?1",
                )?;
                stmt.query_row([id], |row| {
                    let mode_str: String = row.get(4)?;
                    let mode =
                        Mode::from_str(&mode_str).ok_or_else(|| rusqlite::Error::InvalidQuery)?;

                    Ok(Phase {
                        id: row.get(0)?,
                        migration_id: row.get(1)?,
                        order: row.get(2)?,
                        name: row.get(3)?,
                        mode,
                        lua_script: row.get(5)?,
                    })
                })
            })
            .await
            .map_err(|e| match e {
                async_sqlite::Error::Rusqlite(rusqlite::Error::QueryReturnedNoRows) => {
                    RepositoryError::NotFound("Phase", id)
                }
                _ => RepositoryError::Database(e),
            })
    }

    /// Create a new phase.
    pub async fn create_phase(&self, phase: NewPhase) -> Result<i64, RepositoryError> {
        let mode_str = phase.mode.as_str().to_string();
        let migration_id = phase.migration_id;
        let order = phase.order;
        let name = phase.name.clone();
        let lua_script = phase.lua_script.clone();
        let now = Utc::now().to_rfc3339();

        self.client
            .conn_mut(move |conn| {
                // Insert phase
                conn.execute(
                    "INSERT INTO phases (migration_id, \"order\", name, mode, lua_script)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![migration_id, order, name, mode_str, lua_script,],
                )?;
                let phase_id = conn.last_insert_rowid();

                // Update parent migration timestamp
                conn.execute(
                    "UPDATE migrations SET updated_at = ?1 WHERE id = ?2",
                    params![now, migration_id],
                )?;

                Ok(phase_id)
            })
            .await
            .map_err(RepositoryError::Database)
    }

    /// Update a phase.
    pub async fn update_phase(&self, id: i64, update: UpdatePhase) -> Result<(), RepositoryError> {
        let now = Utc::now().to_rfc3339();
        let client = self.client.clone();

        client
            .conn_mut(move |conn| {
                let mut updates = Vec::new();
                let mut param_vals: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

                if let Some(name) = update.name {
                    updates.push("name = ?");
                    param_vals.push(Box::new(name));
                }
                if let Some(mode) = update.mode {
                    updates.push("mode = ?");
                    param_vals.push(Box::new(mode.as_str().to_string()));
                }
                if update.lua_script.is_some() {
                    updates.push("lua_script = ?");
                    param_vals.push(Box::new(update.lua_script));
                }

                if updates.is_empty() {
                    return Ok((0, 0));
                }

                param_vals.push(Box::new(id));

                let sql = format!("UPDATE phases SET {} WHERE id = ?", updates.join(", "));
                let param_refs: Vec<&dyn rusqlite::ToSql> =
                    param_vals.iter().map(|p| p.as_ref()).collect();
                let affected = conn.execute(&sql, param_refs.as_slice())?;

                // Update parent migration timestamp
                let migration_affected = conn.execute(
                    "UPDATE migrations SET updated_at = ?1
                     WHERE id = (SELECT migration_id FROM phases WHERE id = ?2)",
                    params![now, id],
                )?;

                Ok((affected, migration_affected))
            })
            .await
            .map_err(RepositoryError::Database)
            .and_then(|(affected, _)| {
                if affected == 0 {
                    Err(RepositoryError::NotFound("Phase", id))
                } else {
                    Ok(())
                }
            })
    }

    /// Delete a phase.
    pub async fn delete_phase(&self, id: i64) -> Result<(), RepositoryError> {
        let now = Utc::now().to_rfc3339();
        self.client
            .conn_mut(move |conn| {
                // Get migration_id before deleting
                let migration_id: i64 = conn.query_row(
                    "SELECT migration_id FROM phases WHERE id = ?1",
                    [id],
                    |row| row.get(0),
                )?;

                // Delete phase
                let affected = conn.execute("DELETE FROM phases WHERE id = ?1", [id])?;

                if affected > 0 {
                    // Update parent migration timestamp
                    conn.execute(
                        "UPDATE migrations SET updated_at = ?1 WHERE id = ?2",
                        params![now, migration_id],
                    )?;
                }

                Ok(affected)
            })
            .await
            .map_err(RepositoryError::Database)
            .and_then(|affected| {
                if affected == 0 {
                    Err(RepositoryError::NotFound("Phase", id))
                } else {
                    Ok(())
                }
            })
    }

    /// Reorder phases within a migration.
    pub async fn reorder_phases(
        &self,
        migration_id: i64,
        ordered_ids: Vec<i64>,
    ) -> Result<(), RepositoryError> {
        let now = Utc::now().to_rfc3339();
        self.client
            .conn_mut(move |conn| {
                let tx = conn.transaction()?;

                for (index, phase_id) in ordered_ids.iter().enumerate() {
                    tx.execute(
                        "UPDATE phases SET \"order\" = ?1 WHERE id = ?2 AND migration_id = ?3",
                        params![index as i32, phase_id, migration_id],
                    )?;
                }

                // Update parent migration timestamp
                tx.execute(
                    "UPDATE migrations SET updated_at = ?1 WHERE id = ?2",
                    params![now, migration_id],
                )?;

                tx.commit()?;
                Ok(())
            })
            .await
            .map_err(RepositoryError::Database)
    }
}
