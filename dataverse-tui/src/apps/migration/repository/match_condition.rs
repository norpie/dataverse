//! Match condition CRUD operations.

use chrono::Utc;
use rusqlite::params;

use super::super::types::*;
use super::RepositoryError;

/// Input for creating a new match condition.
pub struct NewMatchCondition {
    pub entity_mapping_id: i64,
    pub target_field: String,
    pub order: i32,
}

/// Input for updating a match condition.
pub struct UpdateMatchCondition {
    pub target_field: Option<String>,
}

impl super::MigrationRepository {
    /// Get all match conditions for an entity mapping.
    pub async fn get_match_conditions(
        &self,
        entity_mapping_id: i64,
    ) -> Result<Vec<MatchCondition>, RepositoryError> {
        self.client
            .conn_mut(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, entity_mapping_id, target_field, \"order\"
                     FROM match_conditions
                     WHERE entity_mapping_id = ?1
                     ORDER BY \"order\" ASC",
                )?;
                let rows = stmt.query_map([entity_mapping_id], |row| {
                    Ok(MatchCondition {
                        id: row.get(0)?,
                        entity_mapping_id: row.get(1)?,
                        target_field: row.get(2)?,
                        order: row.get(3)?,
                    })
                })?;
                rows.collect::<Result<Vec<_>, _>>()
            })
            .await
            .map_err(RepositoryError::Database)
    }

    /// Get all match conditions for a migration (joins through entity_mappings, phases).
    pub async fn get_match_conditions_by_migration(
        &self,
        migration_id: i64,
    ) -> Result<Vec<MatchCondition>, RepositoryError> {
        self.client
            .conn_mut(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT mc.id, mc.entity_mapping_id, mc.target_field, mc.\"order\"
                     FROM match_conditions mc
                     INNER JOIN entity_mappings em ON mc.entity_mapping_id = em.id
                     INNER JOIN phases p ON em.phase_id = p.id
                     WHERE p.migration_id = ?1
                     ORDER BY mc.id ASC",
                )?;
                let rows = stmt.query_map([migration_id], |row| {
                    Ok(MatchCondition {
                        id: row.get(0)?,
                        entity_mapping_id: row.get(1)?,
                        target_field: row.get(2)?,
                        order: row.get(3)?,
                    })
                })?;
                rows.collect::<Result<Vec<_>, _>>()
            })
            .await
            .map_err(RepositoryError::Database)
    }

    /// Create a new match condition.
    pub async fn create_match_condition(
        &self,
        condition: NewMatchCondition,
    ) -> Result<i64, RepositoryError> {
        let entity_mapping_id = condition.entity_mapping_id;
        let target_field = condition.target_field;
        let order = condition.order;
        let now = Utc::now().to_rfc3339();

        self.client
            .conn_mut(move |conn| {
                conn.execute(
                    "INSERT INTO match_conditions (entity_mapping_id, target_field, \"order\")
                     VALUES (?1, ?2, ?3)",
                    params![entity_mapping_id, target_field, order],
                )?;
                let condition_id = conn.last_insert_rowid();

                // Update parent migration timestamp
                conn.execute(
                    "UPDATE migrations SET updated_at = ?1
                     WHERE id = (SELECT migration_id FROM phases
                                 WHERE id = (SELECT phase_id FROM entity_mappings WHERE id = ?2))",
                    params![now, entity_mapping_id],
                )?;

                Ok(condition_id)
            })
            .await
            .map_err(RepositoryError::Database)
    }

    /// Update a match condition.
    pub async fn update_match_condition(
        &self,
        id: i64,
        update: UpdateMatchCondition,
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
                    "UPDATE match_conditions SET {} WHERE id = ?",
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
                                             WHERE id = (SELECT entity_mapping_id FROM match_conditions WHERE id = ?2)))",
                    params![now, id],
                )?;

                Ok((affected, migration_affected))
            })
            .await
            .map_err(RepositoryError::Database)
            .and_then(|(affected, _)| {
                if affected == 0 {
                    Err(RepositoryError::NotFound("MatchCondition", id))
                } else {
                    Ok(())
                }
            })
    }

    /// Delete a match condition and all its child transforms.
    pub async fn delete_match_condition(&self, id: i64) -> Result<(), RepositoryError> {
        let now = Utc::now().to_rfc3339();
        self.client
            .conn_mut(move |conn| {
                // Get entity_mapping_id before deleting
                let entity_mapping_id: i64 = conn.query_row(
                    "SELECT entity_mapping_id FROM match_conditions WHERE id = ?1",
                    [id],
                    |row| row.get(0),
                )?;

                // Delete child transforms first (those with parent_type='match_condition' and parent_id=id)
                conn.execute(
                    "DELETE FROM transforms WHERE parent_type = 'match_condition' AND parent_id = ?1",
                    [id],
                )?;

                // Delete the match condition
                let affected = conn.execute("DELETE FROM match_conditions WHERE id = ?1", [id])?;

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
                    Err(RepositoryError::NotFound("MatchCondition", id))
                } else {
                    Ok(())
                }
            })
    }

    /// Delete all match conditions for an entity mapping (and their child transforms).
    pub async fn delete_match_conditions_for_entity_mapping(
        &self,
        entity_mapping_id: i64,
    ) -> Result<(), RepositoryError> {
        let now = Utc::now().to_rfc3339();
        self.client
            .conn_mut(move |conn| {
                // Delete child transforms for all match conditions of this entity mapping
                conn.execute(
                    "DELETE FROM transforms WHERE parent_type = 'match_condition' AND parent_id IN
                     (SELECT id FROM match_conditions WHERE entity_mapping_id = ?1)",
                    [entity_mapping_id],
                )?;

                // Delete all match conditions
                conn.execute(
                    "DELETE FROM match_conditions WHERE entity_mapping_id = ?1",
                    [entity_mapping_id],
                )?;

                // Update parent migration timestamp
                conn.execute(
                    "UPDATE migrations SET updated_at = ?1
                     WHERE id = (SELECT migration_id FROM phases
                                 WHERE id = (SELECT phase_id FROM entity_mappings WHERE id = ?2))",
                    params![now, entity_mapping_id],
                )?;

                Ok(())
            })
            .await
            .map_err(RepositoryError::Database)
    }

    /// Reorder match conditions within an entity mapping.
    pub async fn reorder_match_conditions(
        &self,
        entity_mapping_id: i64,
        ordered_ids: Vec<i64>,
    ) -> Result<(), RepositoryError> {
        let now = Utc::now().to_rfc3339();
        self.client
            .conn_mut(move |conn| {
                let tx = conn.transaction()?;

                for (index, condition_id) in ordered_ids.iter().enumerate() {
                    tx.execute(
                        "UPDATE match_conditions SET \"order\" = ?1 WHERE id = ?2 AND entity_mapping_id = ?3",
                        params![index as i32, condition_id, entity_mapping_id],
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
