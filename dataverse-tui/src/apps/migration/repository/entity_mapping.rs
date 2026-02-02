//! Entity mapping CRUD operations.

use async_sqlite::Client;
use chrono::Utc;
use rusqlite::params;

use crate::widgets::filter_builder::FilterNode;

use super::super::types::*;
use super::helpers::*;
use super::RepositoryError;

/// Input for creating a new entity mapping.
pub struct NewEntityMapping {
    pub phase_id: i64,
    pub order: i32,
    pub source_entity: String,
    pub target_entity: String,
    pub mode: Mode,
    pub lua_script: Option<String>,
    pub match_strategy: MatchStrategy,
    pub match_find_config: Option<FindConfig>,
    pub no_match_fallback: NoMatchFallback,
    pub orphan_strategy: OrphanStrategy,
    pub create_pass_enabled: bool,
    pub update_pass_enabled: bool,
    pub source_filter: Option<FilterNode>,
    pub target_filter: Option<FilterNode>,
    pub test_guids: Option<Vec<String>>,
}

/// Input for updating an entity mapping.
pub struct UpdateEntityMapping {
    pub source_entity: Option<String>,
    pub target_entity: Option<String>,
    pub mode: Option<Mode>,
    pub lua_script: Option<String>,
    pub match_strategy: Option<MatchStrategy>,
    pub match_find_config: Option<FindConfig>,
    pub no_match_fallback: Option<NoMatchFallback>,
    pub orphan_strategy: Option<OrphanStrategy>,
    pub create_pass_enabled: Option<bool>,
    pub update_pass_enabled: Option<bool>,
    pub source_filter: Option<FilterNode>,
    pub target_filter: Option<FilterNode>,
    pub test_guids: Option<Vec<String>>,
}

impl super::MigrationRepository {
    /// Get all entity mappings for a phase (without related data).
    pub async fn get_entity_mappings(
        &self,
        phase_id: i64,
    ) -> Result<Vec<EntityMapping>, RepositoryError> {
        self.client
            .conn(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, phase_id, \"order\", source_entity, target_entity, mode, lua_script,
                            match_strategy, match_find_config, no_match_fallback, orphan_strategy,
                            create_pass_enabled, update_pass_enabled, source_filter, target_filter, test_guids
                     FROM entity_mappings
                     WHERE phase_id = ?1
                     ORDER BY \"order\" ASC",
                )?;
                let rows = stmt.query_map([phase_id], row_to_entity_mapping)?;
                rows.collect::<Result<Vec<_>, _>>()
            })
            .await
            .map_err(RepositoryError::Database)
    }

    /// Get an entity mapping by ID (without related data).
    pub async fn get_entity_mapping(&self, id: i64) -> Result<EntityMapping, RepositoryError> {
        self.client
            .conn(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, phase_id, \"order\", source_entity, target_entity, mode, lua_script,
                            match_strategy, match_find_config, no_match_fallback, orphan_strategy,
                            create_pass_enabled, update_pass_enabled, source_filter, target_filter, test_guids
                     FROM entity_mappings
                     WHERE id = ?1",
                )?;
                stmt.query_row([id], row_to_entity_mapping)
            })
            .await
            .map_err(|e| match e {
                async_sqlite::Error::Rusqlite(rusqlite::Error::QueryReturnedNoRows) => {
                    RepositoryError::NotFound("EntityMapping", id)
                }
                _ => RepositoryError::Database(e),
            })
    }

    /// Create a new entity mapping.
    pub async fn create_entity_mapping(
        &self,
        mapping: NewEntityMapping,
    ) -> Result<i64, RepositoryError> {
        let mode_str = mapping.mode.as_str().to_string();
        let match_strategy_str = mapping.match_strategy.as_str().to_string();
        let no_match_fallback_str = mapping.no_match_fallback.as_str().to_string();
        let orphan_strategy_str = mapping.orphan_strategy.as_str().to_string();

        let match_find_config_blob = mapping
            .match_find_config
            .as_ref()
            .map(serialize_find_config)
            .transpose()?;
        let source_filter_blob = serialize_filter_node(&mapping.source_filter)?;
        let target_filter_blob = serialize_filter_node(&mapping.target_filter)?;
        let test_guids_json = mapping
            .test_guids
            .as_ref()
            .map(|g| serialize_test_guids(g))
            .transpose()?;

        let phase_id = mapping.phase_id;
        let order = mapping.order;
        let source_entity = mapping.source_entity.clone();
        let target_entity = mapping.target_entity.clone();
        let lua_script = mapping.lua_script.clone();
        let create_pass_enabled = mapping.create_pass_enabled;
        let update_pass_enabled = mapping.update_pass_enabled;
        let now = Utc::now().to_rfc3339();

        self.client
            .conn(move |conn| {
                conn.execute(
                    "INSERT INTO entity_mappings (
                        phase_id, \"order\", source_entity, target_entity, mode, lua_script,
                        match_strategy, match_find_config, no_match_fallback, orphan_strategy,
                        create_pass_enabled, update_pass_enabled, source_filter, target_filter, test_guids
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
                    params![
                        phase_id,
                        order,
                        source_entity,
                        target_entity,
                        mode_str,
                        lua_script,
                        match_strategy_str,
                        match_find_config_blob,
                        no_match_fallback_str,
                        orphan_strategy_str,
                        create_pass_enabled as i32,
                        update_pass_enabled as i32,
                        source_filter_blob,
                        target_filter_blob,
                        test_guids_json,
                    ],
                )?;
                let mapping_id = conn.last_insert_rowid();

                // Update parent migration timestamp
                conn.execute(
                    "UPDATE migrations SET updated_at = ?1
                     WHERE id = (SELECT migration_id FROM phases WHERE id = ?2)",
                    params![now, phase_id],
                )?;

                Ok(mapping_id)
            })
            .await
            .map_err(RepositoryError::Database)
    }

    /// Update an entity mapping.
    pub async fn update_entity_mapping(
        &self,
        id: i64,
        update: UpdateEntityMapping,
    ) -> Result<(), RepositoryError> {
        // Serialize complex types before entering closure
        let match_find_config_blob = update
            .match_find_config
            .as_ref()
            .map(serialize_find_config)
            .transpose()?;
        let source_filter_blob = update
            .source_filter
            .as_ref()
            .map(|f| serialize_filter_node(&Some(f.clone())))
            .transpose()?
            .flatten();
        let target_filter_blob = update
            .target_filter
            .as_ref()
            .map(|f| serialize_filter_node(&Some(f.clone())))
            .transpose()?
            .flatten();
        let test_guids_json = update
            .test_guids
            .as_ref()
            .map(|g| serialize_test_guids(g))
            .transpose()?;

        let now = Utc::now().to_rfc3339();
        let client = self.client.clone();

        client
            .conn(move |conn| {
                let mut updates = Vec::new();
                let mut param_vals: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

                if let Some(source_entity) = update.source_entity {
                    updates.push("source_entity = ?");
                    param_vals.push(Box::new(source_entity));
                }
                if let Some(target_entity) = update.target_entity {
                    updates.push("target_entity = ?");
                    param_vals.push(Box::new(target_entity));
                }
                if let Some(mode) = update.mode {
                    updates.push("mode = ?");
                    param_vals.push(Box::new(mode.as_str().to_string()));
                }
                if update.lua_script.is_some() {
                    updates.push("lua_script = ?");
                    param_vals.push(Box::new(update.lua_script));
                }
                if let Some(match_strategy) = update.match_strategy {
                    updates.push("match_strategy = ?");
                    param_vals.push(Box::new(match_strategy.as_str().to_string()));
                }
                if let Some(blob) = match_find_config_blob {
                    updates.push("match_find_config = ?");
                    param_vals.push(Box::new(blob));
                }
                if let Some(no_match_fallback) = update.no_match_fallback {
                    updates.push("no_match_fallback = ?");
                    param_vals.push(Box::new(no_match_fallback.as_str().to_string()));
                }
                if let Some(orphan_strategy) = update.orphan_strategy {
                    updates.push("orphan_strategy = ?");
                    param_vals.push(Box::new(orphan_strategy.as_str().to_string()));
                }
                if let Some(create_pass_enabled) = update.create_pass_enabled {
                    updates.push("create_pass_enabled = ?");
                    param_vals.push(Box::new(create_pass_enabled as i32));
                }
                if let Some(update_pass_enabled) = update.update_pass_enabled {
                    updates.push("update_pass_enabled = ?");
                    param_vals.push(Box::new(update_pass_enabled as i32));
                }
                if let Some(blob) = source_filter_blob {
                    updates.push("source_filter = ?");
                    param_vals.push(Box::new(blob));
                }
                if let Some(blob) = target_filter_blob {
                    updates.push("target_filter = ?");
                    param_vals.push(Box::new(blob));
                }
                if let Some(json) = test_guids_json {
                    updates.push("test_guids = ?");
                    param_vals.push(Box::new(json));
                }

                if updates.is_empty() {
                    return Ok((0, 0));
                }

                param_vals.push(Box::new(id));

                let sql = format!(
                    "UPDATE entity_mappings SET {} WHERE id = ?",
                    updates.join(", ")
                );
                let param_refs: Vec<&dyn rusqlite::ToSql> =
                    param_vals.iter().map(|p| p.as_ref()).collect();
                let affected = conn.execute(&sql, param_refs.as_slice())?;

                // Update parent migration timestamp
                let migration_affected = conn.execute(
                    "UPDATE migrations SET updated_at = ?1
                     WHERE id = (SELECT migration_id FROM phases
                                 WHERE id = (SELECT phase_id FROM entity_mappings WHERE id = ?2))",
                    params![now, id],
                )?;

                Ok((affected, migration_affected))
            })
            .await
            .map_err(RepositoryError::Database)
            .and_then(|(affected, _)| {
                if affected == 0 {
                    Err(RepositoryError::NotFound("EntityMapping", id))
                } else {
                    Ok(())
                }
            })
    }

    /// Delete an entity mapping.
    pub async fn delete_entity_mapping(&self, id: i64) -> Result<(), RepositoryError> {
        let now = Utc::now().to_rfc3339();
        self.client
            .conn(move |conn| {
                // Get phase_id before deleting
                let phase_id: i64 = conn.query_row(
                    "SELECT phase_id FROM entity_mappings WHERE id = ?1",
                    [id],
                    |row| row.get(0),
                )?;

                // Delete entity mapping
                let affected = conn.execute("DELETE FROM entity_mappings WHERE id = ?1", [id])?;

                if affected > 0 {
                    // Update parent migration timestamp
                    conn.execute(
                        "UPDATE migrations SET updated_at = ?1
                         WHERE id = (SELECT migration_id FROM phases WHERE id = ?2)",
                        params![now, phase_id],
                    )?;
                }

                Ok(affected)
            })
            .await
            .map_err(RepositoryError::Database)
            .and_then(|affected| {
                if affected == 0 {
                    Err(RepositoryError::NotFound("EntityMapping", id))
                } else {
                    Ok(())
                }
            })
    }

    /// Reorder entity mappings within a phase.
    pub async fn reorder_entity_mappings(
        &self,
        phase_id: i64,
        ordered_ids: Vec<i64>,
    ) -> Result<(), RepositoryError> {
        let now = Utc::now().to_rfc3339();
        self.client
            .conn_mut(move |conn| {
                let tx = conn.transaction()?;

                for (index, mapping_id) in ordered_ids.iter().enumerate() {
                    tx.execute(
                        "UPDATE entity_mappings SET \"order\" = ?1 WHERE id = ?2 AND phase_id = ?3",
                        params![index as i32, mapping_id, phase_id],
                    )?;
                }

                // Update parent migration timestamp
                tx.execute(
                    "UPDATE migrations SET updated_at = ?1
                     WHERE id = (SELECT migration_id FROM phases WHERE id = ?2)",
                    params![now, phase_id],
                )?;

                tx.commit()?;
                Ok(())
            })
            .await
            .map_err(RepositoryError::Database)
    }
}
