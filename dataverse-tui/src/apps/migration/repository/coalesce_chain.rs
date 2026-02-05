//! Coalesce chain CRUD operations.

use chrono::Utc;
use rusqlite::params;

use super::super::types::*;
use super::RepositoryError;

/// Input for creating a new coalesce chain.
pub struct NewCoalesceChain {
    pub transform_id: i64,
    pub order: i32,
}

impl super::MigrationRepository {
    /// Get all coalesce chains for a transform.
    pub async fn get_coalesce_chains(
        &self,
        transform_id: i64,
    ) -> Result<Vec<CoalesceChain>, RepositoryError> {
        self.client
            .conn_mut(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, transform_id, \"order\"
                     FROM coalesce_chains
                     WHERE transform_id = ?1
                     ORDER BY \"order\" ASC",
                )?;
                let rows = stmt.query_map([transform_id], |row| {
                    Ok(CoalesceChain {
                        id: row.get(0)?,
                        transform_id: row.get(1)?,
                        order: row.get(2)?,
                    })
                })?;
                rows.collect::<Result<Vec<_>, _>>()
            })
            .await
            .map_err(RepositoryError::Database)
    }

    /// Get a coalesce chain by ID.
    pub async fn get_coalesce_chain(&self, id: i64) -> Result<CoalesceChain, RepositoryError> {
        self.client
            .conn_mut(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, transform_id, \"order\"
                     FROM coalesce_chains
                     WHERE id = ?1",
                )?;
                stmt.query_row([id], |row| {
                    Ok(CoalesceChain {
                        id: row.get(0)?,
                        transform_id: row.get(1)?,
                        order: row.get(2)?,
                    })
                })
            })
            .await
            .map_err(|e| match e {
                async_sqlite::Error::Rusqlite(rusqlite::Error::QueryReturnedNoRows) => {
                    RepositoryError::NotFound("CoalesceChain", id)
                }
                _ => RepositoryError::Database(e),
            })
    }

    /// Create a new coalesce chain.
    pub async fn create_coalesce_chain(
        &self,
        chain: NewCoalesceChain,
    ) -> Result<i64, RepositoryError> {
        let transform_id = chain.transform_id;
        let order = chain.order;
        let now = Utc::now().to_rfc3339();

        self.client
            .conn_mut(move |conn| {
                conn.execute(
                    "INSERT INTO coalesce_chains (transform_id, \"order\")
                     VALUES (?1, ?2)",
                    params![transform_id, order],
                )?;
                let chain_id = conn.last_insert_rowid();

                // Update parent migration timestamp
                conn.execute(
                    "UPDATE migrations SET updated_at = ?1
                     WHERE id = (SELECT migration_id FROM phases
                                 WHERE id = (SELECT phase_id FROM entity_mappings
                                             WHERE id = (SELECT entity_mapping_id FROM transforms WHERE id = ?2)))",
                    params![now, transform_id],
                )?;

                Ok(chain_id)
            })
            .await
            .map_err(RepositoryError::Database)
    }

    /// Delete a coalesce chain and all its child transforms.
    pub async fn delete_coalesce_chain(&self, id: i64) -> Result<(), RepositoryError> {
        let now = Utc::now().to_rfc3339();
        self.client
            .conn_mut(move |conn| {
                // Get transform_id before deleting
                let transform_id: i64 = conn.query_row(
                    "SELECT transform_id FROM coalesce_chains WHERE id = ?1",
                    [id],
                    |row| row.get(0),
                )?;

                // Delete child transforms first (those with parent_type='coalesce_chain' and parent_id=id)
                conn.execute(
                    "DELETE FROM transforms WHERE parent_type = 'coalesce_chain' AND parent_id = ?1",
                    [id],
                )?;

                // Delete the coalesce chain
                let affected = conn.execute("DELETE FROM coalesce_chains WHERE id = ?1", [id])?;

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
                    Err(RepositoryError::NotFound("CoalesceChain", id))
                } else {
                    Ok(())
                }
            })
    }

    /// Reorder coalesce chains within a transform.
    pub async fn reorder_coalesce_chains(
        &self,
        transform_id: i64,
        ordered_ids: Vec<i64>,
    ) -> Result<(), RepositoryError> {
        let now = Utc::now().to_rfc3339();
        self.client
            .conn_mut(move |conn| {
                let tx = conn.transaction()?;

                for (index, chain_id) in ordered_ids.iter().enumerate() {
                    tx.execute(
                        "UPDATE coalesce_chains SET \"order\" = ?1 WHERE id = ?2 AND transform_id = ?3",
                        params![index as i32, chain_id, transform_id],
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
