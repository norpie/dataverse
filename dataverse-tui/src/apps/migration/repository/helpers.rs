//! Helper functions for serialization and row conversion.

use chrono::DateTime;
use chrono::Utc;
use rusqlite::Row;

use crate::widgets::filter_builder::FilterNode;

use super::super::types::*;
use super::RepositoryError;

// =============================================================================
// Error Bridging
// =============================================================================

/// Convert a `RepositoryError` into a `rusqlite::Error`, preserving the full error details.
///
/// This is used inside rusqlite row-mapping closures that must return `rusqlite::Error`.
pub fn repo_err_to_rusqlite(e: RepositoryError) -> rusqlite::Error {
    rusqlite::Error::ToSqlConversionFailure(Box::new(e))
}

/// Create a `rusqlite::Error` for an invalid enum value, preserving what was received.
pub fn invalid_enum(type_name: &str, value: &str) -> rusqlite::Error {
    repo_err_to_rusqlite(RepositoryError::InvalidEnum(format!(
        "unknown {type_name}: {value:?}"
    )))
}

// =============================================================================
// Datetime Parsing
// =============================================================================

/// Parse ISO 8601 datetime string.
pub fn parse_datetime(s: &str) -> Result<DateTime<Utc>, rusqlite::Error> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| {
            repo_err_to_rusqlite(RepositoryError::Deserialization(format!(
                "invalid datetime {s:?}: {e}"
            )))
        })
}

// =============================================================================
// Bincode Serialization
// =============================================================================

/// Serialize FilterNode to bincode.
pub fn serialize_filter_node(
    node: &Option<FilterNode>,
) -> Result<Option<Vec<u8>>, RepositoryError> {
    node.as_ref()
        .map(|n| bincode::serde::encode_to_vec(n, bincode::config::standard()))
        .transpose()
        .map_err(|e| RepositoryError::Serialization(e.to_string()))
}

/// Deserialize FilterNode from bincode.
pub fn deserialize_filter_node(bytes: &[u8]) -> Result<FilterNode, RepositoryError> {
    bincode::serde::decode_from_slice(bytes, bincode::config::standard())
        .map(|(node, _)| node)
        .map_err(|e| RepositoryError::Deserialization(e.to_string()))
}

/// Serialize Condition to bincode.
pub fn serialize_condition(condition: &Condition) -> Result<Vec<u8>, RepositoryError> {
    bincode::serde::encode_to_vec(condition, bincode::config::standard())
        .map_err(|e| RepositoryError::Serialization(e.to_string()))
}

/// Deserialize Condition from bincode.
pub fn deserialize_condition(bytes: &[u8]) -> Result<Condition, RepositoryError> {
    bincode::serde::decode_from_slice(bytes, bincode::config::standard())
        .map(|(condition, _)| condition)
        .map_err(|e| RepositoryError::Deserialization(e.to_string()))
}

/// Serialize TransformData to bincode.
pub fn serialize_transform_data(data: &TransformData) -> Result<Vec<u8>, RepositoryError> {
    bincode::serde::encode_to_vec(data, bincode::config::standard())
        .map_err(|e| RepositoryError::Serialization(e.to_string()))
}

/// Deserialize TransformData from bincode.
pub fn deserialize_transform_data(bytes: &[u8]) -> Result<TransformData, RepositoryError> {
    bincode::serde::decode_from_slice(bytes, bincode::config::standard())
        .map(|(data, _)| data)
        .map_err(|e| RepositoryError::Deserialization(e.to_string()))
}

/// Serialize ValueType to bincode.
pub fn serialize_value_type(
    value_type: &dataverse_lib::model::ValueType,
) -> Result<Vec<u8>, RepositoryError> {
    bincode::serde::encode_to_vec(value_type, bincode::config::standard())
        .map_err(|e| RepositoryError::Serialization(e.to_string()))
}

/// Deserialize ValueType from bincode.
pub fn deserialize_value_type(
    bytes: &[u8],
) -> Result<dataverse_lib::model::ValueType, RepositoryError> {
    bincode::serde::decode_from_slice(bytes, bincode::config::standard())
        .map(|(vt, _)| vt)
        .map_err(|e| RepositoryError::Deserialization(e.to_string()))
}

/// Serialize FindConfig to bincode.
pub fn serialize_find_config(config: &FindConfig) -> Result<Vec<u8>, RepositoryError> {
    bincode::serde::encode_to_vec(config, bincode::config::standard())
        .map_err(|e| RepositoryError::Serialization(e.to_string()))
}

/// Deserialize FindConfig from bincode.
pub fn deserialize_find_config(bytes: &[u8]) -> Result<FindConfig, RepositoryError> {
    bincode::serde::decode_from_slice(bytes, bincode::config::standard())
        .map(|(config, _)| config)
        .map_err(|e| RepositoryError::Deserialization(e.to_string()))
}

// =============================================================================
// JSON Serialization
// =============================================================================

/// Serialize test GUIDs to comma-delimited string.
pub fn serialize_test_guids(guids: &[String]) -> Result<String, RepositoryError> {
    Ok(guids.join(","))
}

/// Deserialize test GUIDs from comma-delimited string.
pub fn deserialize_test_guids(csv: &str) -> Result<Vec<String>, RepositoryError> {
    if csv.trim().is_empty() {
        return Ok(Vec::new());
    }
    Ok(csv.split(',').map(|s| s.trim().to_string()).collect())
}

/// Serialize queue item IDs to JSON.
pub fn serialize_queue_item_ids(ids: &[i64]) -> Result<String, RepositoryError> {
    serde_json::to_string(ids).map_err(RepositoryError::Json)
}

/// Deserialize queue item IDs from JSON.
pub fn deserialize_queue_item_ids(json: &str) -> Result<Vec<i64>, RepositoryError> {
    serde_json::from_str(json).map_err(RepositoryError::Json)
}

// =============================================================================
// Transform Type Helper
// =============================================================================

/// Get transform type string from TransformData.
pub fn transform_type_str(data: &TransformData) -> String {
    match data {
        TransformData::Copy { .. } => "copy",
        TransformData::Constant { .. } => "constant",
        TransformData::Guard { .. } => "guard",
        TransformData::Match { .. } => "match",
        TransformData::Find { .. } => "find",
        TransformData::Format { .. } => "format",
        TransformData::Replace { .. } => "replace",
        TransformData::StringOps { .. } => "string_ops",
        TransformData::ValueMap { .. } => "value_map",
        TransformData::Math { .. } => "math",
        TransformData::Coalesce => "coalesce",
        TransformData::Convert { .. } => "convert",
        TransformData::ParseInt => "parse_int",
        TransformData::ParseDecimal => "parse_decimal",
        TransformData::ParseDate { .. } => "parse_date",
        TransformData::Guid => "guid",
    }
    .to_string()
}

// =============================================================================
// Row Converters
// =============================================================================

/// Convert database row to EntityMapping.
/// Column order: id, phase_id, order, name, source_entity, target_entity, mode, lua_script,
///               match_strategy, match_find_config, no_match_fallback, orphan_strategy,
///               create_pass_enabled, update_pass_enabled, delete_pass_enabled, deactivate_pass_enabled,
///               associate_pass_enabled, disassociate_pass_enabled, source_filter, target_filter, test_guids
pub fn row_to_entity_mapping(row: &Row) -> Result<EntityMapping, rusqlite::Error> {
    let mode_str: String = row.get(6)?;
    let mode = Mode::from_str(&mode_str).ok_or_else(|| invalid_enum("Mode", &mode_str))?;

    let match_strategy_str: String = row.get(8)?;
    let match_strategy = MatchStrategy::from_str(&match_strategy_str)
        .ok_or_else(|| invalid_enum("MatchStrategy", &match_strategy_str))?;

    let no_match_fallback_str: String = row.get(10)?;
    let no_match_fallback = NoMatchFallback::from_str(&no_match_fallback_str)
        .ok_or_else(|| invalid_enum("NoMatchFallback", &no_match_fallback_str))?;

    let orphan_strategy_str: String = row.get(11)?;
    let orphan_strategy = OrphanStrategy::from_str(&orphan_strategy_str)
        .ok_or_else(|| invalid_enum("OrphanStrategy", &orphan_strategy_str))?;

    let match_find_config_blob: Option<Vec<u8>> = row.get(9)?;
    let match_find_config = match_find_config_blob
        .map(|b| deserialize_find_config(&b))
        .transpose()
        .map_err(repo_err_to_rusqlite)?;

    let source_filter_blob: Option<Vec<u8>> = row.get(18)?;
    let source_filter = source_filter_blob
        .map(|b| deserialize_filter_node(&b))
        .transpose()
        .map_err(repo_err_to_rusqlite)?;

    let target_filter_blob: Option<Vec<u8>> = row.get(19)?;
    let target_filter = target_filter_blob
        .map(|b| deserialize_filter_node(&b))
        .transpose()
        .map_err(repo_err_to_rusqlite)?;

    let test_guids_json: Option<String> = row.get(20)?;
    let test_guids = test_guids_json
        .map(|j| deserialize_test_guids(&j))
        .transpose()
        .map_err(repo_err_to_rusqlite)?;

    Ok(EntityMapping {
        id: row.get(0)?,
        phase_id: row.get(1)?,
        order: row.get(2)?,
        name: row.get(3)?,
        source_entity: row.get(4)?,
        target_entity: row.get(5)?,
        mode,
        lua_script: row.get(7)?,
        match_strategy,
        match_find_config,
        no_match_fallback,
        orphan_strategy,
        create_pass_enabled: row.get::<_, i32>(12)? != 0,
        update_pass_enabled: row.get::<_, i32>(13)? != 0,
        delete_pass_enabled: row.get::<_, i32>(14)? != 0,
        deactivate_pass_enabled: row.get::<_, i32>(15)? != 0,
        associate_pass_enabled: row.get::<_, i32>(16)? != 0,
        disassociate_pass_enabled: row.get::<_, i32>(17)? != 0,
        source_filter,
        target_filter,
        test_guids,
    })
}

/// Convert database row to Transform.
pub fn row_to_transform(row: &Row) -> Result<Transform, rusqlite::Error> {
    let parent_type_str: String = row.get(2)?;
    let parent_type = ParentType::from_str(&parent_type_str)
        .ok_or_else(|| invalid_enum("ParentType", &parent_type_str))?;

    let data_blob: Vec<u8> = row.get(6)?;
    let data = deserialize_transform_data(&data_blob).map_err(repo_err_to_rusqlite)?;

    Ok(Transform {
        id: row.get(0)?,
        entity_mapping_id: row.get(1)?,
        parent_type,
        parent_id: row.get(3)?,
        order: row.get(4)?,
        data,
    })
}

/// Convert database row to PhaseRun.
pub fn row_to_phase_run(row: &Row) -> Result<PhaseRun, rusqlite::Error> {
    let status_str: String = row.get(4)?;
    let status = PhaseRunStatus::from_str(&status_str)
        .ok_or_else(|| invalid_enum("PhaseRunStatus", &status_str))?;

    let completed_at_str: Option<String> = row.get(3)?;
    let completed_at = completed_at_str.map(|s| parse_datetime(&s)).transpose()?;

    let queue_item_ids_json: Option<String> = row.get(5)?;
    let queue_item_ids = queue_item_ids_json
        .map(|j| deserialize_queue_item_ids(&j))
        .transpose()
        .map_err(repo_err_to_rusqlite)?;

    Ok(PhaseRun {
        id: row.get(0)?,
        phase_id: row.get(1)?,
        started_at: parse_datetime(&row.get::<_, String>(2)?)?,
        completed_at,
        status,
        queue_item_ids,
        error: row.get(6)?,
    })
}

/// Convert database row to MatchBranch.
/// Column order: id, transform_id, order, condition
pub fn row_to_match_branch(row: &Row) -> Result<MatchBranch, rusqlite::Error> {
    let condition_blob: Vec<u8> = row.get(3)?;
    let condition = deserialize_condition(&condition_blob).map_err(repo_err_to_rusqlite)?;

    Ok(MatchBranch {
        id: row.get(0)?,
        transform_id: row.get(1)?,
        order: row.get(2)?,
        condition,
    })
}
