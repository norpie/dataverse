//! Field mapping CRUD operations.

use async_sqlite::Client;
use chrono::Utc;
use rusqlite::params;

use super::super::types::*;
use super::RepositoryError;

/// Input for creating a new field mapping.
pub struct NewFieldMapping {
    pub entity_mapping_id: i64,
    pub order: i32,
    pub target_field: String,
}

/// Input for updating a field mapping.
pub struct UpdateFieldMapping {
    pub target_field: Option<String>,
}

impl super::MigrationRepository {
    /// Get all field mappings for an entity mapping.
    pub async fn get_field_mappings(
        &self,
        entity_mapping_id: i64,
    ) -> Result<Vec<FieldMapping>, RepositoryError> {
        self.client
            .conn_mut(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, entity_mapping_id, \"order\", target_field
                     FROM field_mappings
                     WHERE entity_mapping_id = ?1
                     ORDER BY \"order\" ASC",
                )?;
                let rows = stmt.query_map([entity_mapping_id], |row| {
                    Ok(FieldMapping {
                        id: row.get(0)?,
                        entity_mapping_id: row.get(1)?,
                        order: row.get(2)?,
                        target_field: row.get(3)?,
                    })
                })?;
                rows.collect::<Result<Vec<_>, _>>()
            })
            .await
            .map_err(RepositoryError::Database)
    }

    /// Get all field mappings for a migration (joins through entity_mappings and phases).
    pub async fn get_field_mappings_by_migration(
        &self,
        migration_id: i64,
    ) -> Result<Vec<FieldMapping>, RepositoryError> {
        self.client
            .conn_mut(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT fm.id, fm.entity_mapping_id, fm.\"order\", fm.target_field
                     FROM field_mappings fm
                     INNER JOIN entity_mappings em ON fm.entity_mapping_id = em.id
                     INNER JOIN phases p ON em.phase_id = p.id
                     WHERE p.migration_id = ?1
                     ORDER BY p.\"order\" ASC, em.\"order\" ASC, fm.\"order\" ASC",
                )?;
                let rows = stmt.query_map([migration_id], |row| {
                    Ok(FieldMapping {
                        id: row.get(0)?,
                        entity_mapping_id: row.get(1)?,
                        order: row.get(2)?,
                        target_field: row.get(3)?,
                    })
                })?;
                rows.collect::<Result<Vec<_>, _>>()
            })
            .await
            .map_err(RepositoryError::Database)
    }

    /// Get a field mapping by ID.
    pub async fn get_field_mapping(&self, id: i64) -> Result<FieldMapping, RepositoryError> {
        self.client
            .conn_mut(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, entity_mapping_id, \"order\", target_field
                     FROM field_mappings
                     WHERE id = ?1",
                )?;
                stmt.query_row([id], |row| {
                    Ok(FieldMapping {
                        id: row.get(0)?,
                        entity_mapping_id: row.get(1)?,
                        order: row.get(2)?,
                        target_field: row.get(3)?,
                    })
                })
            })
            .await
            .map_err(|e| match e {
                async_sqlite::Error::Rusqlite(rusqlite::Error::QueryReturnedNoRows) => {
                    RepositoryError::NotFound("FieldMapping", id)
                }
                _ => RepositoryError::Database(e),
            })
    }

    /// Create a new field mapping.
    pub async fn create_field_mapping(
        &self,
        mapping: NewFieldMapping,
    ) -> Result<i64, RepositoryError> {
        let entity_mapping_id = mapping.entity_mapping_id;
        let order = mapping.order;
        let target_field = mapping.target_field.clone();
        let now = Utc::now().to_rfc3339();

        self.client
            .conn_mut(move |conn| {
                conn.execute(
                    "INSERT INTO field_mappings (entity_mapping_id, \"order\", target_field)
                     VALUES (?1, ?2, ?3)",
                    params![entity_mapping_id, order, target_field],
                )?;
                let mapping_id = conn.last_insert_rowid();

                // Update parent migration timestamp
                conn.execute(
                    "UPDATE migrations SET updated_at = ?1
                     WHERE id = (SELECT migration_id FROM phases
                                 WHERE id = (SELECT phase_id FROM entity_mappings WHERE id = ?2))",
                    params![now, entity_mapping_id],
                )?;

                Ok(mapping_id)
            })
            .await
            .map_err(RepositoryError::Database)
    }

    /// Update a field mapping.
    pub async fn update_field_mapping(
        &self,
        id: i64,
        update: UpdateFieldMapping,
    ) -> Result<(), RepositoryError> {
        let now = Utc::now().to_rfc3339();
        let client = self.client.clone();

        client
            .conn_mut(move |conn| {
                if let Some(target_field) = update.target_field {
                    let affected = conn.execute(
                        "UPDATE field_mappings SET target_field = ?1 WHERE id = ?2",
                        params![target_field, id],
                    )?;

                    if affected > 0 {
                        // Update parent migration timestamp
                        conn.execute(
                            "UPDATE migrations SET updated_at = ?1
                             WHERE id = (SELECT migration_id FROM phases
                                         WHERE id = (SELECT phase_id FROM entity_mappings
                                                     WHERE id = (SELECT entity_mapping_id FROM field_mappings WHERE id = ?2)))",
                            params![now, id],
                        )?;
                    }

                    Ok(affected)
                } else {
                    Ok(0)
                }
            })
            .await
            .map_err(RepositoryError::Database)
            .and_then(|affected| {
                if affected == 0 {
                    Err(RepositoryError::NotFound("FieldMapping", id))
                } else {
                    Ok(())
                }
            })
    }

    /// Delete a field mapping.
    pub async fn delete_field_mapping(&self, id: i64) -> Result<(), RepositoryError> {
        let now = Utc::now().to_rfc3339();
        self.client
            .conn_mut(move |conn| {
                // Get entity_mapping_id before deleting
                let entity_mapping_id: i64 = conn.query_row(
                    "SELECT entity_mapping_id FROM field_mappings WHERE id = ?1",
                    [id],
                    |row| row.get(0),
                )?;

                // Delete field mapping
                let affected = conn.execute("DELETE FROM field_mappings WHERE id = ?1", [id])?;

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
                    Err(RepositoryError::NotFound("FieldMapping", id))
                } else {
                    Ok(())
                }
            })
    }

    /// Reorder field mappings within an entity mapping.
    pub async fn reorder_field_mappings(
        &self,
        entity_mapping_id: i64,
        ordered_ids: Vec<i64>,
    ) -> Result<(), RepositoryError> {
        let now = Utc::now().to_rfc3339();
        self.client
            .conn_mut(move |conn| {
                let tx = conn.transaction()?;

                for (index, mapping_id) in ordered_ids.iter().enumerate() {
                    tx.execute(
                        "UPDATE field_mappings SET \"order\" = ?1 WHERE id = ?2 AND entity_mapping_id = ?3",
                        params![index as i32, mapping_id, entity_mapping_id],
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
