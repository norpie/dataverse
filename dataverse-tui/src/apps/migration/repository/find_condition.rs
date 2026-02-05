//! Find condition CRUD operations.

use chrono::Utc;
use rusqlite::params;

use super::super::types::*;
use super::RepositoryError;

/// Input for creating a new find condition.
pub struct NewFindCondition {
    pub transform_id: i64,
    pub target_field: String,
    pub order: i32,
}

/// Input for updating a find condition.
pub struct UpdateFindCondition {
    pub target_field: Option<String>,
}

impl super::MigrationRepository {
    /// Get all find conditions for a transform.
    pub async fn get_find_conditions(
        &self,
        transform_id: i64,
    ) -> Result<Vec<FindCondition>, RepositoryError> {
        self.client
            .conn_mut(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, transform_id, target_field, \"order\"
                     FROM find_conditions
                     WHERE transform_id = ?1
                     ORDER BY \"order\" ASC",
                )?;
                let rows = stmt.query_map([transform_id], |row| {
                    Ok(FindCondition {
                        id: row.get(0)?,
                        transform_id: row.get(1)?,
                        target_field: row.get(2)?,
                        order: row.get(3)?,
                    })
                })?;
                rows.collect::<Result<Vec<_>, _>>()
            })
            .await
            .map_err(RepositoryError::Database)
    }

    /// Get all find conditions for a migration (joins through transforms, entity_mappings, phases).
    pub async fn get_find_conditions_by_migration(
        &self,
        migration_id: i64,
    ) -> Result<Vec<FindCondition>, RepositoryError> {
        self.client
            .conn_mut(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT fc.id, fc.transform_id, fc.target_field, fc.\"order\"
                     FROM find_conditions fc
                     INNER JOIN transforms t ON fc.transform_id = t.id
                     INNER JOIN entity_mappings em ON t.entity_mapping_id = em.id
                     INNER JOIN phases p ON em.phase_id = p.id
                     WHERE p.migration_id = ?1
                     ORDER BY fc.id ASC",
                )?;
                let rows = stmt.query_map([migration_id], |row| {
                    Ok(FindCondition {
                        id: row.get(0)?,
                        transform_id: row.get(1)?,
                        target_field: row.get(2)?,
                        order: row.get(3)?,
                    })
                })?;
                rows.collect::<Result<Vec<_>, _>>()
            })
            .await
            .map_err(RepositoryError::Database)
    }

    /// Get a find condition by ID.
    pub async fn get_find_condition(&self, id: i64) -> Result<FindCondition, RepositoryError> {
        self.client
            .conn_mut(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, transform_id, target_field, \"order\"
                     FROM find_conditions
                     WHERE id = ?1",
                )?;
                stmt.query_row([id], |row| {
                    Ok(FindCondition {
                        id: row.get(0)?,
                        transform_id: row.get(1)?,
                        target_field: row.get(2)?,
                        order: row.get(3)?,
                    })
                })
            })
            .await
            .map_err(|e| match e {
                async_sqlite::Error::Rusqlite(rusqlite::Error::QueryReturnedNoRows) => {
                    RepositoryError::NotFound("FindCondition", id)
                }
                _ => RepositoryError::Database(e),
            })
    }

    /// Create a new find condition.
    pub async fn create_find_condition(
        &self,
        condition: NewFindCondition,
    ) -> Result<i64, RepositoryError> {
        let transform_id = condition.transform_id;
        let target_field = condition.target_field;
        let order = condition.order;
        let now = Utc::now().to_rfc3339();

        self.client
            .conn_mut(move |conn| {
                conn.execute(
                    "INSERT INTO find_conditions (transform_id, target_field, \"order\")
                     VALUES (?1, ?2, ?3)",
                    params![transform_id, target_field, order],
                )?;
                let condition_id = conn.last_insert_rowid();

                // Update parent migration timestamp
                conn.execute(
                    "UPDATE migrations SET updated_at = ?1
                     WHERE id = (SELECT migration_id FROM phases
                                 WHERE id = (SELECT phase_id FROM entity_mappings
                                             WHERE id = (SELECT entity_mapping_id FROM transforms WHERE id = ?2)))",
                    params![now, transform_id],
                )?;

                Ok(condition_id)
            })
            .await
            .map_err(RepositoryError::Database)
    }

    /// Update a find condition.
    pub async fn update_find_condition(
        &self,
        id: i64,
        update: UpdateFindCondition,
    ) -> Result<(), RepositoryError> {
        let now = Utc::now().to_rfc3339();

        self.client
            .conn_mut(move |conn| {
                let mut updates = Vec::new();
                let mut param_vals: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

                if let Some(target_field) = update.target_field {
                    updates.push("target_field = ?");
                    param_vals.push(Box::new(target_field));
                }

                if updates.is_empty() {
                    return Ok((0, 0));
                }

                param_vals.push(Box::new(id));

                let sql = format!(
                    "UPDATE find_conditions SET {} WHERE id = ?",
                    updates.join(", ")
                );
                let param_refs: Vec<&dyn rusqlite::ToSql> =
                    param_vals.iter().map(|p| p.as_ref()).collect();
                let affected = conn.execute(&sql, param_refs.as_slice())?;

                // Update parent migration timestamp
                let migration_affected = conn.execute(
                    "UPDATE migrations SET updated_at = ?1
                     WHERE id = (SELECT migration_id FROM phases
                                 WHERE id = (SELECT phase_id FROM entity_mappings
                                             WHERE id = (SELECT entity_mapping_id FROM transforms
                                                         WHERE id = (SELECT transform_id FROM find_conditions WHERE id = ?2))))",
                    params![now, id],
                )?;

                Ok((affected, migration_affected))
            })
            .await
            .map_err(RepositoryError::Database)
            .and_then(|(affected, _)| {
                if affected == 0 {
                    Err(RepositoryError::NotFound("FindCondition", id))
                } else {
                    Ok(())
                }
            })
    }

    /// Delete a find condition and all its child transforms.
    pub async fn delete_find_condition(&self, id: i64) -> Result<(), RepositoryError> {
        let now = Utc::now().to_rfc3339();
        self.client
            .conn_mut(move |conn| {
                // Get transform_id before deleting
                let transform_id: i64 = conn.query_row(
                    "SELECT transform_id FROM find_conditions WHERE id = ?1",
                    [id],
                    |row| row.get(0),
                )?;

                // Delete child transforms first (those with parent_type='find_condition' and parent_id=id)
                conn.execute(
                    "DELETE FROM transforms WHERE parent_type = 'find_condition' AND parent_id = ?1",
                    [id],
                )?;

                // Delete the find condition
                let affected = conn.execute("DELETE FROM find_conditions WHERE id = ?1", [id])?;

                if affected > 0 {
                    // Update parent migration timestamp
                    conn.execute(
                        "UPDATE migrations SET updated_at = ?1
                         WHERE id = (SELECT migration_id FROM phases
                                     WHERE id = (SELECT phase_id FROM entity_mappings
                                                 WHERE id = (SELECT entity_mapping_id FROM transforms WHERE id = ?2)))",
                        params![now, transform_id],
                    )?;
                }

                Ok(affected)
            })
            .await
            .map_err(RepositoryError::Database)
            .and_then(|affected| {
                if affected == 0 {
                    Err(RepositoryError::NotFound("FindCondition", id))
                } else {
                    Ok(())
                }
            })
    }

    /// Reorder find conditions within a transform.
    pub async fn reorder_find_conditions(
        &self,
        transform_id: i64,
        ordered_ids: Vec<i64>,
    ) -> Result<(), RepositoryError> {
        let now = Utc::now().to_rfc3339();
        self.client
            .conn_mut(move |conn| {
                let tx = conn.transaction()?;

                for (index, condition_id) in ordered_ids.iter().enumerate() {
                    tx.execute(
                        "UPDATE find_conditions SET \"order\" = ?1 WHERE id = ?2 AND transform_id = ?3",
                        params![index as i32, condition_id, transform_id],
                    )?;
                }

                // Update parent migration timestamp
                tx.execute(
                    "UPDATE migrations SET updated_at = ?1
                     WHERE id = (SELECT migration_id FROM phases
                                 WHERE id = (SELECT phase_id FROM entity_mappings
                                             WHERE id = (SELECT entity_mapping_id FROM transforms WHERE id = ?2)))",
                    params![now, transform_id],
                )?;

                tx.commit()?;
                Ok(())
            })
            .await
            .map_err(RepositoryError::Database)
    }
}
