//! Variable CRUD operations.

use async_sqlite::Client;
use chrono::Utc;
use rusqlite::params;

use dataverse_lib::model::ValueType;

use super::super::types::*;
use super::RepositoryError;
use super::helpers::{deserialize_value_type, repo_err_to_rusqlite, serialize_value_type};

/// Input for creating a new variable.
pub struct NewVariable {
    pub entity_mapping_id: i64,
    pub order: i32,
    pub name: String,
    pub declared_type: ValueType,
}

/// Input for updating a variable.
pub struct UpdateVariable {
    pub name: Option<String>,
    pub declared_type: Option<ValueType>,
}

impl super::MigrationRepository {
    /// Get all variables for an entity mapping.
    pub async fn get_variables(
        &self,
        entity_mapping_id: i64,
    ) -> Result<Vec<Variable>, RepositoryError> {
        self.client
            .conn_mut(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, entity_mapping_id, \"order\", name, declared_type
                     FROM variables
                     WHERE entity_mapping_id = ?1
                     ORDER BY \"order\" ASC",
                )?;
                let rows = stmt.query_map([entity_mapping_id], |row| {
                    let declared_type_blob: Vec<u8> = row.get(4)?;
                    let declared_type = deserialize_value_type(&declared_type_blob)
                        .map_err(repo_err_to_rusqlite)?;
                    Ok(Variable {
                        id: row.get(0)?,
                        entity_mapping_id: row.get(1)?,
                        order: row.get(2)?,
                        name: row.get(3)?,
                        declared_type,
                    })
                })?;
                rows.collect::<Result<Vec<_>, _>>()
            })
            .await
            .map_err(RepositoryError::Database)
    }

    /// Get all variables for a migration (joins through entity_mappings and phases).
    pub async fn get_variables_by_migration(
        &self,
        migration_id: i64,
    ) -> Result<Vec<Variable>, RepositoryError> {
        self.client
            .conn_mut(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT v.id, v.entity_mapping_id, v.\"order\", v.name, v.declared_type
                     FROM variables v
                     INNER JOIN entity_mappings em ON v.entity_mapping_id = em.id
                     INNER JOIN phases p ON em.phase_id = p.id
                     WHERE p.migration_id = ?1
                     ORDER BY p.\"order\" ASC, em.\"order\" ASC, v.\"order\" ASC",
                )?;
                let rows = stmt.query_map([migration_id], |row| {
                    let declared_type_blob: Vec<u8> = row.get(4)?;
                    let declared_type = deserialize_value_type(&declared_type_blob)
                        .map_err(repo_err_to_rusqlite)?;
                    Ok(Variable {
                        id: row.get(0)?,
                        entity_mapping_id: row.get(1)?,
                        order: row.get(2)?,
                        name: row.get(3)?,
                        declared_type,
                    })
                })?;
                rows.collect::<Result<Vec<_>, _>>()
            })
            .await
            .map_err(RepositoryError::Database)
    }

    /// Get a variable by ID.
    pub async fn get_variable(&self, id: i64) -> Result<Variable, RepositoryError> {
        self.client
            .conn_mut(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, entity_mapping_id, \"order\", name, declared_type
                     FROM variables
                     WHERE id = ?1",
                )?;
                stmt.query_row([id], |row| {
                    let declared_type_blob: Vec<u8> = row.get(4)?;
                    let declared_type = deserialize_value_type(&declared_type_blob)
                        .map_err(repo_err_to_rusqlite)?;
                    Ok(Variable {
                        id: row.get(0)?,
                        entity_mapping_id: row.get(1)?,
                        order: row.get(2)?,
                        name: row.get(3)?,
                        declared_type,
                    })
                })
            })
            .await
            .map_err(|e| match e {
                async_sqlite::Error::Rusqlite(rusqlite::Error::QueryReturnedNoRows) => {
                    RepositoryError::NotFound("Variable", id)
                }
                _ => RepositoryError::Database(e),
            })
    }

    /// Create a new variable.
    pub async fn create_variable(&self, variable: NewVariable) -> Result<i64, RepositoryError> {
        let entity_mapping_id = variable.entity_mapping_id;
        let order = variable.order;
        let name = variable.name.clone();
        let declared_type_blob = serialize_value_type(&variable.declared_type)?;
        let now = Utc::now().to_rfc3339();

        self.client
            .conn_mut(move |conn| {
                conn.execute(
                    "INSERT INTO variables (entity_mapping_id, \"order\", name, declared_type)
                     VALUES (?1, ?2, ?3, ?4)",
                    params![entity_mapping_id, order, name, declared_type_blob],
                )?;
                let variable_id = conn.last_insert_rowid();

                // Update parent migration timestamp
                conn.execute(
                    "UPDATE migrations SET updated_at = ?1
                     WHERE id = (SELECT migration_id FROM phases
                                 WHERE id = (SELECT phase_id FROM entity_mappings WHERE id = ?2))",
                    params![now, entity_mapping_id],
                )?;

                Ok(variable_id)
            })
            .await
            .map_err(RepositoryError::Database)
    }

    /// Update a variable.
    pub async fn update_variable(
        &self,
        id: i64,
        update: UpdateVariable,
    ) -> Result<(), RepositoryError> {
        let declared_type_blob = update
            .declared_type
            .as_ref()
            .map(serialize_value_type)
            .transpose()?;
        let now = Utc::now().to_rfc3339();
        let client = self.client.clone();

        client
            .conn_mut(move |conn| {
                let mut affected = 0;

                if let Some(name) = update.name {
                    affected += conn.execute(
                        "UPDATE variables SET name = ?1 WHERE id = ?2",
                        params![name, id],
                    )?;
                }

                if let Some(blob) = declared_type_blob {
                    affected += conn.execute(
                        "UPDATE variables SET declared_type = ?1 WHERE id = ?2",
                        params![blob, id],
                    )?;
                }

                if affected > 0 {
                    // Update parent migration timestamp
                    conn.execute(
                        "UPDATE migrations SET updated_at = ?1
                         WHERE id = (SELECT migration_id FROM phases
                                     WHERE id = (SELECT phase_id FROM entity_mappings
                                                 WHERE id = (SELECT entity_mapping_id FROM variables WHERE id = ?2)))",
                        params![now, id],
                    )?;
                }

                Ok(affected)
            })
            .await
            .map_err(RepositoryError::Database)
            .and_then(|affected| {
                if affected == 0 {
                    Err(RepositoryError::NotFound("Variable", id))
                } else {
                    Ok(())
                }
            })
    }

    /// Delete a variable.
    pub async fn delete_variable(&self, id: i64) -> Result<(), RepositoryError> {
        let now = Utc::now().to_rfc3339();
        self.client
            .conn_mut(move |conn| {
                // Get entity_mapping_id before deleting
                let entity_mapping_id: i64 = conn.query_row(
                    "SELECT entity_mapping_id FROM variables WHERE id = ?1",
                    [id],
                    |row| row.get(0),
                )?;

                // Delete variable
                let affected = conn.execute("DELETE FROM variables WHERE id = ?1", [id])?;

                if affected > 0 {
                    // Update parent migration timestamp
                    conn.execute(
                        "UPDATE migrations SET updated_at = ?1
                         WHERE id = (SELECT migration_id FROM phases
                                     WHERE id = (SELECT phase_id FROM entity_mappings WHERE id = ?2))",
                        params![now, entity_mapping_id],
                    )?;
                }

                Ok(affected)
            })
            .await
            .map_err(RepositoryError::Database)
            .and_then(|affected| {
                if affected == 0 {
                    Err(RepositoryError::NotFound("Variable", id))
                } else {
                    Ok(())
                }
            })
    }

    /// Reorder variables within an entity mapping.
    pub async fn reorder_variables(
        &self,
        entity_mapping_id: i64,
        ordered_ids: Vec<i64>,
    ) -> Result<(), RepositoryError> {
        let now = Utc::now().to_rfc3339();
        self.client
            .conn_mut(move |conn| {
                let tx = conn.transaction()?;

                for (index, variable_id) in ordered_ids.iter().enumerate() {
                    tx.execute(
                        "UPDATE variables SET \"order\" = ?1 WHERE id = ?2 AND entity_mapping_id = ?3",
                        params![index as i32, variable_id, entity_mapping_id],
                    )?;
                }

                // Update parent migration timestamp
                tx.execute(
                    "UPDATE migrations SET updated_at = ?1
                     WHERE id = (SELECT migration_id FROM phases
                                 WHERE id = (SELECT phase_id FROM entity_mappings WHERE id = ?2))",
                    params![now, entity_mapping_id],
                )?;

                tx.commit()?;
                Ok(())
            })
            .await
            .map_err(RepositoryError::Database)
    }
}
