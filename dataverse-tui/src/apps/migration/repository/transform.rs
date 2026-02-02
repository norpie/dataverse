//! Transform and match branch CRUD operations.

use async_sqlite::Client;
use chrono::Utc;
use rusqlite::params;

use super::super::types::*;
use super::helpers::*;
use super::RepositoryError;

/// Input for creating a new transform.
pub struct NewTransform {
    pub entity_mapping_id: i64,
    pub parent_type: ParentType,
    pub parent_id: i64,
    pub order: i32,
    pub data: TransformData,
}

/// Input for updating a transform.
pub struct UpdateTransform {
    pub data: TransformData,
}

/// Input for creating a new match branch.
pub struct NewMatchBranch {
    pub transform_id: i64,
    pub order: i32,
    pub condition: Option<Condition>,
    pub is_default: bool,
}

/// Input for updating a match branch.
pub struct UpdateMatchBranch {
    pub condition: Option<Condition>,
    pub is_default: Option<bool>,
}

impl super::MigrationRepository {
    // =========================================================================
    // Transform Operations
    // =========================================================================

    /// Get all transforms for a specific parent.
    pub async fn get_transforms(
        &self,
        parent_type: ParentType,
        parent_id: i64,
    ) -> Result<Vec<Transform>, RepositoryError> {
        let parent_type_str = parent_type.as_str().to_string();
        self.client
            .conn_mut(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, entity_mapping_id, parent_type, parent_id, \"order\", transform_type, data
                     FROM transforms
                     WHERE parent_type = ?1 AND parent_id = ?2
                     ORDER BY \"order\" ASC",
                )?;
                let rows = stmt.query_map(params![parent_type_str, parent_id], row_to_transform)?;
                rows.collect::<Result<Vec<_>, _>>()
            })
            .await
            .map_err(RepositoryError::Database)
    }

    /// Get a transform by ID.
    pub async fn get_transform(&self, id: i64) -> Result<Transform, RepositoryError> {
        self.client
            .conn_mut(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, entity_mapping_id, parent_type, parent_id, \"order\", transform_type, data
                     FROM transforms
                     WHERE id = ?1",
                )?;
                stmt.query_row([id], row_to_transform)
            })
            .await
            .map_err(|e| match e {
                async_sqlite::Error::Rusqlite(rusqlite::Error::QueryReturnedNoRows) => {
                    RepositoryError::NotFound("Transform", id)
                }
                _ => RepositoryError::Database(e),
            })
    }

    /// Create a new transform.
    pub async fn create_transform(
        &self,
        transform: NewTransform,
    ) -> Result<i64, RepositoryError> {
        let parent_type_str = transform.parent_type.as_str().to_string();
        let transform_type = transform_type_str(&transform.data);
        let data_blob = serialize_transform_data(&transform.data)?;
        let entity_mapping_id = transform.entity_mapping_id;
        let parent_id = transform.parent_id;
        let order = transform.order;
        let now = Utc::now().to_rfc3339();

        self.client
            .conn_mut(move |conn| {
                conn.execute(
                    "INSERT INTO transforms (entity_mapping_id, parent_type, parent_id, \"order\", transform_type, data)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        entity_mapping_id,
                        parent_type_str,
                        parent_id,
                        order,
                        transform_type,
                        data_blob,
                    ],
                )?;
                let transform_id = conn.last_insert_rowid();

                // Update parent migration timestamp
                conn.execute(
                    "UPDATE migrations SET updated_at = ?1
                     WHERE id = (SELECT migration_id FROM phases
                                 WHERE id = (SELECT phase_id FROM entity_mappings WHERE id = ?2))",
                    params![now, entity_mapping_id],
                )?;

                Ok(transform_id)
            })
            .await
            .map_err(RepositoryError::Database)
    }

    /// Update a transform.
    pub async fn update_transform(
        &self,
        id: i64,
        update: UpdateTransform,
    ) -> Result<(), RepositoryError> {
        let transform_type = transform_type_str(&update.data);
        let data_blob = serialize_transform_data(&update.data)?;
        let now = Utc::now().to_rfc3339();

        self.client
            .conn_mut(move |conn| {
                let affected = conn.execute(
                    "UPDATE transforms SET transform_type = ?1, data = ?2 WHERE id = ?3",
                    params![transform_type, data_blob, id],
                )?;

                if affected > 0 {
                    // Update parent migration timestamp
                    conn.execute(
                        "UPDATE migrations SET updated_at = ?1
                         WHERE id = (SELECT migration_id FROM phases
                                     WHERE id = (SELECT phase_id FROM entity_mappings
                                                 WHERE id = (SELECT entity_mapping_id FROM transforms WHERE id = ?2)))",
                        params![now, id],
                    )?;
                }

                Ok(affected)
            })
            .await
            .map_err(RepositoryError::Database)
            .and_then(|affected| {
                if affected == 0 {
                    Err(RepositoryError::NotFound("Transform", id))
                } else {
                    Ok(())
                }
            })
    }

    /// Delete a transform.
    pub async fn delete_transform(&self, id: i64) -> Result<(), RepositoryError> {
        let now = Utc::now().to_rfc3339();
        self.client
            .conn_mut(move |conn| {
                // Get entity_mapping_id before deleting
                let entity_mapping_id: i64 = conn.query_row(
                    "SELECT entity_mapping_id FROM transforms WHERE id = ?1",
                    [id],
                    |row| row.get(0),
                )?;

                // Delete transform (cascades to match branches and their child transforms)
                let affected = conn.execute("DELETE FROM transforms WHERE id = ?1", [id])?;

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
                    Err(RepositoryError::NotFound("Transform", id))
                } else {
                    Ok(())
                }
            })
    }

    /// Reorder transforms within a parent.
    pub async fn reorder_transforms(
        &self,
        parent_type: ParentType,
        parent_id: i64,
        ordered_ids: Vec<i64>,
    ) -> Result<(), RepositoryError> {
        let parent_type_str = parent_type.as_str().to_string();
        let now = Utc::now().to_rfc3339();

        self.client
            .conn_mut(move |conn| {
                let tx = conn.transaction()?;

                // Get entity_mapping_id for timestamp update (if transforms exist)
                let entity_mapping_id: Option<i64> = tx
                    .query_row(
                        "SELECT entity_mapping_id FROM transforms WHERE parent_type = ?1 AND parent_id = ?2 LIMIT 1",
                        params![parent_type_str.clone(), parent_id],
                        |row| row.get(0),
                    )
                    .ok();

                for (index, transform_id) in ordered_ids.iter().enumerate() {
                    tx.execute(
                        "UPDATE transforms SET \"order\" = ?1 WHERE id = ?2 AND parent_type = ?3 AND parent_id = ?4",
                        params![index as i32, transform_id, parent_type_str, parent_id],
                    )?;
                }

                // Update parent migration timestamp if we found transforms
                if let Some(entity_mapping_id) = entity_mapping_id {
                    tx.execute(
                        "UPDATE migrations SET updated_at = ?1
                         WHERE id = (SELECT migration_id FROM phases
                                     WHERE id = (SELECT phase_id FROM entity_mappings WHERE id = ?2))",
                        params![now, entity_mapping_id],
                    )?;
                }

                tx.commit()?;
                Ok(())
            })
            .await
            .map_err(RepositoryError::Database)
    }

    // =========================================================================
    // Match Branch Operations
    // =========================================================================

    /// Get all match branches for a transform.
    pub async fn get_match_branches(
        &self,
        transform_id: i64,
    ) -> Result<Vec<MatchBranch>, RepositoryError> {
        self.client
            .conn_mut(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, transform_id, \"order\", condition, is_default
                     FROM match_branches
                     WHERE transform_id = ?1
                     ORDER BY \"order\" ASC",
                )?;
                let rows = stmt.query_map([transform_id], |row| {
                    let condition_blob: Option<Vec<u8>> = row.get(3)?;
                    let condition = condition_blob
                        .map(|b| deserialize_condition(&b))
                        .transpose()
                        .map_err(|_| rusqlite::Error::InvalidQuery)?;

                    Ok(MatchBranch {
                        id: row.get(0)?,
                        transform_id: row.get(1)?,
                        order: row.get(2)?,
                        condition,
                        is_default: row.get::<_, i32>(4)? != 0,
                    })
                })?;
                rows.collect::<Result<Vec<_>, _>>()
            })
            .await
            .map_err(RepositoryError::Database)
    }

    /// Get a match branch by ID.
    pub async fn get_match_branch(&self, id: i64) -> Result<MatchBranch, RepositoryError> {
        self.client
            .conn_mut(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, transform_id, \"order\", condition, is_default
                     FROM match_branches
                     WHERE id = ?1",
                )?;
                stmt.query_row([id], |row| {
                    let condition_blob: Option<Vec<u8>> = row.get(3)?;
                    let condition = condition_blob
                        .map(|b| deserialize_condition(&b))
                        .transpose()
                        .map_err(|_| rusqlite::Error::InvalidQuery)?;

                    Ok(MatchBranch {
                        id: row.get(0)?,
                        transform_id: row.get(1)?,
                        order: row.get(2)?,
                        condition,
                        is_default: row.get::<_, i32>(4)? != 0,
                    })
                })
            })
            .await
            .map_err(|e| match e {
                async_sqlite::Error::Rusqlite(rusqlite::Error::QueryReturnedNoRows) => {
                    RepositoryError::NotFound("MatchBranch", id)
                }
                _ => RepositoryError::Database(e),
            })
    }

    /// Create a new match branch.
    pub async fn create_match_branch(
        &self,
        branch: NewMatchBranch,
    ) -> Result<i64, RepositoryError> {
        let condition_blob = branch
            .condition
            .as_ref()
            .map(serialize_condition)
            .transpose()?;
        let transform_id = branch.transform_id;
        let order = branch.order;
        let is_default = branch.is_default;
        let now = Utc::now().to_rfc3339();

        self.client
            .conn_mut(move |conn| {
                conn.execute(
                    "INSERT INTO match_branches (transform_id, \"order\", condition, is_default)
                     VALUES (?1, ?2, ?3, ?4)",
                    params![transform_id, order, condition_blob, is_default as i32,],
                )?;
                let branch_id = conn.last_insert_rowid();

                // Update parent migration timestamp
                conn.execute(
                    "UPDATE migrations SET updated_at = ?1
                     WHERE id = (SELECT migration_id FROM phases
                                 WHERE id = (SELECT phase_id FROM entity_mappings
                                             WHERE id = (SELECT entity_mapping_id FROM transforms WHERE id = ?2)))",
                    params![now, transform_id],
                )?;

                Ok(branch_id)
            })
            .await
            .map_err(RepositoryError::Database)
    }

    /// Update a match branch.
    pub async fn update_match_branch(
        &self,
        id: i64,
        update: UpdateMatchBranch,
    ) -> Result<(), RepositoryError> {
        // Serialize before entering closure
        let condition_blob = update
            .condition
            .as_ref()
            .map(serialize_condition)
            .transpose()?;

        let now = Utc::now().to_rfc3339();
        let client = self.client.clone();

        client
            .conn(move |conn| {
                let mut updates = Vec::new();
                let mut param_vals: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

                if let Some(blob) = condition_blob {
                    updates.push("condition = ?");
                    param_vals.push(Box::new(blob));
                }
                if let Some(is_default) = update.is_default {
                    updates.push("is_default = ?");
                    param_vals.push(Box::new(is_default as i32));
                }

                if updates.is_empty() {
                    return Ok((0, 0));
                }

                param_vals.push(Box::new(id));

                let sql = format!(
                    "UPDATE match_branches SET {} WHERE id = ?",
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
                                                         WHERE id = (SELECT transform_id FROM match_branches WHERE id = ?2))))",
                    params![now, id],
                )?;

                Ok((affected, migration_affected))
            })
            .await
            .map_err(RepositoryError::Database)
            .and_then(|(affected, _)| {
                if affected == 0 {
                    Err(RepositoryError::NotFound("MatchBranch", id))
                } else {
                    Ok(())
                }
            })
    }

    /// Delete a match branch.
    pub async fn delete_match_branch(&self, id: i64) -> Result<(), RepositoryError> {
        let now = Utc::now().to_rfc3339();
        self.client
            .conn_mut(move |conn| {
                // Get transform_id before deleting
                let transform_id: i64 = conn.query_row(
                    "SELECT transform_id FROM match_branches WHERE id = ?1",
                    [id],
                    |row| row.get(0),
                )?;

                // Delete match branch
                let affected = conn.execute("DELETE FROM match_branches WHERE id = ?1", [id])?;

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
                    Err(RepositoryError::NotFound("MatchBranch", id))
                } else {
                    Ok(())
                }
            })
    }

    /// Reorder match branches within a transform.
    pub async fn reorder_match_branches(
        &self,
        transform_id: i64,
        ordered_ids: Vec<i64>,
    ) -> Result<(), RepositoryError> {
        let now = Utc::now().to_rfc3339();
        self.client
            .conn_mut(move |conn| {
                let tx = conn.transaction()?;

                for (index, branch_id) in ordered_ids.iter().enumerate() {
                    tx.execute(
                        "UPDATE match_branches SET \"order\" = ?1 WHERE id = ?2 AND transform_id = ?3",
                        params![index as i32, branch_id, transform_id],
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
