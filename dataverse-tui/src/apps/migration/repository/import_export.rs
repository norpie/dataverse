//! Portable migration import/export support.

use chrono::Utc;
use rusqlite::params;
use serde::Deserialize;
use serde::Serialize;

use crate::credentials::Environment;
use crate::widgets::filter_builder::FilterNode;

use super::super::types::*;
use super::RepositoryError;
use super::helpers::*;

const EXPORT_FORMAT: &str = "dataverse-migration";
const EXPORT_VERSION: u32 = 1;

/// Portable migration export bundle.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MigrationExportBundle {
    pub format: String,
    pub version: u32,
    pub exported_at: String,
    pub source_environment: ExportEnvironment,
    pub target_environment: ExportEnvironment,
    pub migration: ExportMigration,
}

/// Environment metadata captured for import-time matching.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExportEnvironment {
    pub display_name: String,
    pub url: String,
}

/// Migration configuration payload.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExportMigration {
    pub name: String,
    pub description: Option<String>,
    pub phases: Vec<ExportPhase>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExportPhase {
    pub name: String,
    pub mode: Mode,
    pub lua_script: Option<String>,
    pub entity_mappings: Vec<ExportEntityMapping>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExportEntityMapping {
    pub name: String,
    pub source_entity: String,
    pub target_entity: String,
    pub mode: Mode,
    pub lua_script: Option<String>,
    pub match_strategy: MatchStrategy,
    pub match_find_config: Option<FindConfig>,
    pub match_lua_script: Option<String>,
    pub no_match_fallback: NoMatchFallback,
    pub orphan_strategy: OrphanStrategy,
    pub create_pass_enabled: bool,
    pub activate_pass_enabled: bool,
    pub update_pass_enabled: bool,
    pub delete_pass_enabled: bool,
    pub deactivate_pass_enabled: bool,
    pub associate_pass_enabled: bool,
    pub disassociate_pass_enabled: bool,
    pub source_filter: Option<FilterNode>,
    pub target_filter: Option<FilterNode>,
    pub test_guids: Option<Vec<String>>,
    pub variables: Vec<ExportVariable>,
    pub field_mappings: Vec<ExportFieldMapping>,
    pub match_conditions: Vec<ExportMatchCondition>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExportVariable {
    pub name: String,
    pub declared_type: dataverse_lib::model::ValueType,
    pub transforms: Vec<ExportTransform>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExportFieldMapping {
    pub target_field: String,
    pub transforms: Vec<ExportTransform>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExportMatchCondition {
    pub target_field: String,
    pub transforms: Vec<ExportTransform>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExportTransform {
    pub data: TransformData,
    pub children: Vec<ExportTransformChild>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ExportTransformChild {
    GuardFallback {
        transforms: Vec<ExportTransform>,
    },
    MatchBranch {
        condition: Condition,
        transforms: Vec<ExportTransform>,
    },
    MatchDefault {
        transforms: Vec<ExportTransform>,
    },
    CoalesceChain {
        transforms: Vec<ExportTransform>,
    },
    FindCondition {
        target_field: String,
        transforms: Vec<ExportTransform>,
    },
    FindDefault {
        transforms: Vec<ExportTransform>,
    },
}

struct ExportRows {
    phases: Vec<Phase>,
    entity_mappings: Vec<EntityMapping>,
    variables: Vec<Variable>,
    field_mappings: Vec<FieldMapping>,
    transforms: Vec<Transform>,
    match_branches: Vec<MatchBranch>,
    coalesce_chains: Vec<CoalesceChain>,
    find_conditions: Vec<FindCondition>,
    match_conditions: Vec<MatchCondition>,
}

impl super::MigrationRepository {
    /// Export a migration as a portable configuration bundle.
    pub async fn export_migration(
        &self,
        id: i64,
        environments: &[Environment],
    ) -> Result<MigrationExportBundle, RepositoryError> {
        let migration = self.get_migration(id).await?;
        let rows = ExportRows {
            phases: self.get_phases(id).await?,
            entity_mappings: self.get_entity_mappings_by_migration(id).await?,
            variables: self.get_variables_by_migration(id).await?,
            field_mappings: self.get_field_mappings_by_migration(id).await?,
            transforms: self.get_transforms_by_migration(id).await?,
            match_branches: self.get_match_branches_by_migration(id).await?,
            coalesce_chains: self.get_coalesce_chains_by_migration(id).await?,
            find_conditions: self.get_find_conditions_by_migration(id).await?,
            match_conditions: self.get_match_conditions_by_migration(id).await?,
        };

        let source_environment = export_environment(migration.source_environment_id, environments);
        let target_environment = export_environment(migration.target_environment_id, environments);
        let phases = export_phases(&rows);

        Ok(MigrationExportBundle {
            format: EXPORT_FORMAT.to_string(),
            version: EXPORT_VERSION,
            exported_at: Utc::now().to_rfc3339(),
            source_environment,
            target_environment,
            migration: ExportMigration {
                name: migration.name,
                description: migration.description,
                phases,
            },
        })
    }

    /// Import a portable migration bundle as a new migration.
    pub async fn import_migration(
        &self,
        bundle: MigrationExportBundle,
        source_environment_id: i64,
        target_environment_id: i64,
        name: String,
    ) -> Result<i64, RepositoryError> {
        if bundle.format != EXPORT_FORMAT || bundle.version != EXPORT_VERSION {
            return Err(RepositoryError::Deserialization(format!(
                "unsupported migration export {} version {}",
                bundle.format, bundle.version
            )));
        }

        let now = Utc::now().to_rfc3339();
        self.client
            .conn_mut(move |conn| {
                let tx = conn.transaction()?;
                tx.execute(
                    "INSERT INTO migrations (name, description, source_environment_id, target_environment_id, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        name,
                        bundle.migration.description,
                        source_environment_id,
                        target_environment_id,
                        now,
                        now,
                    ],
                )?;
                let migration_id = tx.last_insert_rowid();

                for (phase_order, phase) in bundle.migration.phases.iter().enumerate() {
                    tx.execute(
                        "INSERT INTO phases (migration_id, \"order\", name, mode, lua_script)
                         VALUES (?1, ?2, ?3, ?4, ?5)",
                        params![
                            migration_id,
                            phase_order as i32,
                            phase.name,
                            phase.mode.as_str(),
                            phase.lua_script,
                        ],
                    )?;
                    let phase_id = tx.last_insert_rowid();

                    for (mapping_order, mapping) in phase.entity_mappings.iter().enumerate() {
                        let mapping_id = insert_entity_mapping(&tx, phase_id, mapping_order, mapping)?;

                        for (variable_order, variable) in mapping.variables.iter().enumerate() {
                            let declared_type = serialize_value_type(&variable.declared_type)
                                .map_err(repo_err_to_rusqlite)?;
                            tx.execute(
                                "INSERT INTO variables (entity_mapping_id, \"order\", name, declared_type)
                                 VALUES (?1, ?2, ?3, ?4)",
                                params![mapping_id, variable_order as i32, variable.name, declared_type],
                            )?;
                            let variable_id = tx.last_insert_rowid();
                            insert_transform_chain(
                                &tx,
                                mapping_id,
                                ParentType::Variable,
                                variable_id,
                                &variable.transforms,
                            )?;
                        }

                        for (field_order, field) in mapping.field_mappings.iter().enumerate() {
                            tx.execute(
                                "INSERT INTO field_mappings (entity_mapping_id, \"order\", target_field)
                                 VALUES (?1, ?2, ?3)",
                                params![mapping_id, field_order as i32, field.target_field],
                            )?;
                            let field_id = tx.last_insert_rowid();
                            insert_transform_chain(
                                &tx,
                                mapping_id,
                                ParentType::FieldMapping,
                                field_id,
                                &field.transforms,
                            )?;
                        }

                        for (condition_order, condition) in mapping.match_conditions.iter().enumerate() {
                            tx.execute(
                                "INSERT INTO match_conditions (entity_mapping_id, target_field, \"order\")
                                 VALUES (?1, ?2, ?3)",
                                params![mapping_id, condition.target_field, condition_order as i32],
                            )?;
                            let condition_id = tx.last_insert_rowid();
                            insert_transform_chain(
                                &tx,
                                mapping_id,
                                ParentType::MatchCondition,
                                condition_id,
                                &condition.transforms,
                            )?;
                        }
                    }
                }

                tx.commit()?;
                Ok(migration_id)
            })
            .await
            .map_err(RepositoryError::Database)
    }
}

fn export_environment(id: i64, environments: &[Environment]) -> ExportEnvironment {
    environments
        .iter()
        .find(|env| env.id == id)
        .map(|env| ExportEnvironment {
            display_name: env.display_name.clone(),
            url: env.url.clone(),
        })
        .unwrap_or_else(|| ExportEnvironment {
            display_name: format!("#{}", id),
            url: String::new(),
        })
}

fn export_phases(rows: &ExportRows) -> Vec<ExportPhase> {
    rows.phases
        .iter()
        .map(|phase| ExportPhase {
            name: phase.name.clone(),
            mode: phase.mode,
            lua_script: phase.lua_script.clone(),
            entity_mappings: rows
                .entity_mappings
                .iter()
                .filter(|mapping| mapping.phase_id == phase.id)
                .map(|mapping| export_entity_mapping(mapping, rows))
                .collect(),
        })
        .collect()
}

fn export_entity_mapping(mapping: &EntityMapping, rows: &ExportRows) -> ExportEntityMapping {
    ExportEntityMapping {
        name: mapping.name.clone(),
        source_entity: mapping.source_entity.clone(),
        target_entity: mapping.target_entity.clone(),
        mode: mapping.mode,
        lua_script: mapping.lua_script.clone(),
        match_strategy: mapping.match_strategy,
        match_find_config: mapping.match_find_config.clone().map(|mut config| {
            config.transform_chain_root_id = None;
            config
        }),
        match_lua_script: mapping.match_lua_script.clone(),
        no_match_fallback: mapping.no_match_fallback,
        orphan_strategy: mapping.orphan_strategy,
        create_pass_enabled: mapping.create_pass_enabled,
        activate_pass_enabled: mapping.activate_pass_enabled,
        update_pass_enabled: mapping.update_pass_enabled,
        delete_pass_enabled: mapping.delete_pass_enabled,
        deactivate_pass_enabled: mapping.deactivate_pass_enabled,
        associate_pass_enabled: mapping.associate_pass_enabled,
        disassociate_pass_enabled: mapping.disassociate_pass_enabled,
        source_filter: mapping.source_filter.clone(),
        target_filter: mapping.target_filter.clone(),
        test_guids: mapping.test_guids.clone(),
        variables: rows
            .variables
            .iter()
            .filter(|variable| variable.entity_mapping_id == mapping.id)
            .map(|variable| ExportVariable {
                name: variable.name.clone(),
                declared_type: variable.declared_type.clone(),
                transforms: export_transform_chain(rows, ParentType::Variable, variable.id),
            })
            .collect(),
        field_mappings: rows
            .field_mappings
            .iter()
            .filter(|field| field.entity_mapping_id == mapping.id)
            .map(|field| ExportFieldMapping {
                target_field: field.target_field.clone(),
                transforms: export_transform_chain(rows, ParentType::FieldMapping, field.id),
            })
            .collect(),
        match_conditions: {
            let mut conditions: Vec<_> = rows
                .match_conditions
                .iter()
                .filter(|condition| condition.entity_mapping_id == mapping.id)
                .collect();
            conditions.sort_by_key(|condition| condition.order);
            conditions
                .into_iter()
                .map(|condition| ExportMatchCondition {
                    target_field: condition.target_field.clone(),
                    transforms: export_transform_chain(
                        rows,
                        ParentType::MatchCondition,
                        condition.id,
                    ),
                })
                .collect()
        },
    }
}

fn export_transform_chain(
    rows: &ExportRows,
    parent_type: ParentType,
    parent_id: i64,
) -> Vec<ExportTransform> {
    rows.transforms
        .iter()
        .filter(|transform| {
            transform.parent_type == parent_type && transform.parent_id == parent_id
        })
        .map(|transform| export_transform(transform, rows))
        .collect()
}

fn export_transform(transform: &Transform, rows: &ExportRows) -> ExportTransform {
    let children = match &transform.data {
        TransformData::Guard { .. } => export_guard_children(transform.id, rows),
        TransformData::Match { has_default } => {
            export_match_children(transform.id, *has_default, rows)
        }
        TransformData::Coalesce => export_coalesce_children(transform.id, rows),
        TransformData::Find { fallback, mode, .. } => {
            export_find_children(transform.id, fallback, mode, rows)
        }
        _ => Vec::new(),
    };

    ExportTransform {
        data: transform.data.clone(),
        children,
    }
}

fn export_guard_children(transform_id: i64, rows: &ExportRows) -> Vec<ExportTransformChild> {
    let transforms = export_transform_chain(rows, ParentType::GuardFallback, transform_id);
    if transforms.is_empty() {
        Vec::new()
    } else {
        vec![ExportTransformChild::GuardFallback { transforms }]
    }
}

fn export_match_children(
    transform_id: i64,
    has_default: bool,
    rows: &ExportRows,
) -> Vec<ExportTransformChild> {
    let mut branches: Vec<_> = rows
        .match_branches
        .iter()
        .filter(|branch| branch.transform_id == transform_id)
        .collect();
    branches.sort_by_key(|branch| branch.order);

    let mut children: Vec<ExportTransformChild> = branches
        .into_iter()
        .map(|branch| ExportTransformChild::MatchBranch {
            condition: branch.condition.clone(),
            transforms: export_transform_chain(rows, ParentType::MatchBranch, branch.id),
        })
        .collect();

    if has_default {
        children.push(ExportTransformChild::MatchDefault {
            transforms: export_transform_chain(rows, ParentType::MatchDefault, transform_id),
        });
    }

    children
}

fn export_coalesce_children(transform_id: i64, rows: &ExportRows) -> Vec<ExportTransformChild> {
    let mut chains: Vec<_> = rows
        .coalesce_chains
        .iter()
        .filter(|chain| chain.transform_id == transform_id)
        .collect();
    chains.sort_by_key(|chain| chain.order);

    chains
        .into_iter()
        .map(|chain| ExportTransformChild::CoalesceChain {
            transforms: export_transform_chain(rows, ParentType::CoalesceChain, chain.id),
        })
        .collect()
}

fn export_find_children(
    transform_id: i64,
    fallback: &FindFallback,
    mode: &FindMode,
    rows: &ExportRows,
) -> Vec<ExportTransformChild> {
    let mut children = Vec::new();

    if matches!(mode, FindMode::Where) {
        let mut conditions: Vec<_> = rows
            .find_conditions
            .iter()
            .filter(|condition| condition.transform_id == transform_id)
            .collect();
        conditions.sort_by_key(|condition| condition.order);
        children.extend(conditions.into_iter().map(|condition| {
            ExportTransformChild::FindCondition {
                target_field: condition.target_field.clone(),
                transforms: export_transform_chain(rows, ParentType::FindCondition, condition.id),
            }
        }));
    }

    if matches!(fallback, FindFallback::Default) {
        children.push(ExportTransformChild::FindDefault {
            transforms: export_transform_chain(rows, ParentType::FindDefault, transform_id),
        });
    }

    children
}

fn insert_entity_mapping(
    tx: &rusqlite::Transaction<'_>,
    phase_id: i64,
    mapping_order: usize,
    mapping: &ExportEntityMapping,
) -> Result<i64, rusqlite::Error> {
    let match_find_config = mapping
        .match_find_config
        .as_ref()
        .map(serialize_find_config)
        .transpose()
        .map_err(repo_err_to_rusqlite)?;
    let source_filter =
        serialize_filter_node(&mapping.source_filter).map_err(repo_err_to_rusqlite)?;
    let target_filter =
        serialize_filter_node(&mapping.target_filter).map_err(repo_err_to_rusqlite)?;
    let test_guids = mapping
        .test_guids
        .as_ref()
        .map(|guids| serialize_test_guids(guids))
        .transpose()
        .map_err(repo_err_to_rusqlite)?;

    tx.execute(
        "INSERT INTO entity_mappings (
            phase_id, \"order\", name, source_entity, target_entity, mode, lua_script,
            match_strategy, match_find_config, match_lua_script,
            no_match_fallback, orphan_strategy,
            create_pass_enabled, activate_pass_enabled, update_pass_enabled,
            delete_pass_enabled, deactivate_pass_enabled,
            associate_pass_enabled, disassociate_pass_enabled,
            source_filter, target_filter, test_guids
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22)",
        params![
            phase_id,
            mapping_order as i32,
            mapping.name,
            mapping.source_entity,
            mapping.target_entity,
            mapping.mode.as_str(),
            mapping.lua_script,
            mapping.match_strategy.as_str(),
            match_find_config,
            mapping.match_lua_script,
            mapping.no_match_fallback.as_str(),
            mapping.orphan_strategy.as_str(),
            mapping.create_pass_enabled as i32,
            mapping.activate_pass_enabled as i32,
            mapping.update_pass_enabled as i32,
            mapping.delete_pass_enabled as i32,
            mapping.deactivate_pass_enabled as i32,
            mapping.associate_pass_enabled as i32,
            mapping.disassociate_pass_enabled as i32,
            source_filter,
            target_filter,
            test_guids,
        ],
    )?;

    Ok(tx.last_insert_rowid())
}

fn insert_transform_chain(
    tx: &rusqlite::Transaction<'_>,
    entity_mapping_id: i64,
    parent_type: ParentType,
    parent_id: i64,
    transforms: &[ExportTransform],
) -> Result<(), rusqlite::Error> {
    for (order, transform) in transforms.iter().enumerate() {
        let data = serialize_transform_data(&transform.data).map_err(repo_err_to_rusqlite)?;
        tx.execute(
            "INSERT INTO transforms (entity_mapping_id, parent_type, parent_id, \"order\", transform_type, data)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                entity_mapping_id,
                parent_type.as_str(),
                parent_id,
                order as i32,
                transform_type_str(&transform.data),
                data,
            ],
        )?;
        let transform_id = tx.last_insert_rowid();
        insert_transform_children(tx, entity_mapping_id, transform_id, &transform.children)?;
    }

    Ok(())
}

fn insert_transform_children(
    tx: &rusqlite::Transaction<'_>,
    entity_mapping_id: i64,
    transform_id: i64,
    children: &[ExportTransformChild],
) -> Result<(), rusqlite::Error> {
    let mut branch_order = 0;
    let mut coalesce_order = 0;
    let mut find_order = 0;

    for child in children {
        match child {
            ExportTransformChild::GuardFallback { transforms } => insert_transform_chain(
                tx,
                entity_mapping_id,
                ParentType::GuardFallback,
                transform_id,
                transforms,
            )?,
            ExportTransformChild::MatchBranch {
                condition,
                transforms,
            } => {
                let condition = serialize_condition(condition).map_err(repo_err_to_rusqlite)?;
                tx.execute(
                    "INSERT INTO match_branches (transform_id, \"order\", condition)
                     VALUES (?1, ?2, ?3)",
                    params![transform_id, branch_order, condition],
                )?;
                branch_order += 1;
                let branch_id = tx.last_insert_rowid();
                insert_transform_chain(
                    tx,
                    entity_mapping_id,
                    ParentType::MatchBranch,
                    branch_id,
                    transforms,
                )?;
            }
            ExportTransformChild::MatchDefault { transforms } => insert_transform_chain(
                tx,
                entity_mapping_id,
                ParentType::MatchDefault,
                transform_id,
                transforms,
            )?,
            ExportTransformChild::CoalesceChain { transforms } => {
                tx.execute(
                    "INSERT INTO coalesce_chains (transform_id, \"order\") VALUES (?1, ?2)",
                    params![transform_id, coalesce_order],
                )?;
                coalesce_order += 1;
                let chain_id = tx.last_insert_rowid();
                insert_transform_chain(
                    tx,
                    entity_mapping_id,
                    ParentType::CoalesceChain,
                    chain_id,
                    transforms,
                )?;
            }
            ExportTransformChild::FindCondition {
                target_field,
                transforms,
            } => {
                tx.execute(
                    "INSERT INTO find_conditions (transform_id, target_field, \"order\")
                     VALUES (?1, ?2, ?3)",
                    params![transform_id, target_field, find_order],
                )?;
                find_order += 1;
                let condition_id = tx.last_insert_rowid();
                insert_transform_chain(
                    tx,
                    entity_mapping_id,
                    ParentType::FindCondition,
                    condition_id,
                    transforms,
                )?;
            }
            ExportTransformChild::FindDefault { transforms } => insert_transform_chain(
                tx,
                entity_mapping_id,
                ParentType::FindDefault,
                transform_id,
                transforms,
            )?,
        }
    }

    Ok(())
}
