//! Target matching — match each source record to a target record.
//!
//! Supports three strategies:
//! - **SameID**: Match by primary key (source ID == target ID).
//! - **Find**: Execute match condition source chains, then scan target records
//!   for matches using `values_equal` + `traverse_path`.
//! - **Lua**: Run a Lua script once for all records, producing a source→target
//!   GUID mapping. Per-record matching is then an O(1) lookup.

use std::collections::HashMap;
use std::sync::Arc;

use dataverse_lib::model::Entity;
use dataverse_lib::model::Record;
use dataverse_lib::model::Value;
use mlua::Table;
use uuid::Uuid;

use crate::apps::migration::engine::execute_chain;
use crate::apps::migration::engine::util::traverse_path;
use crate::apps::migration::engine::util::values_equal;
use crate::apps::migration::engine::ChainItem;
use crate::apps::migration::engine::FindCache;
use crate::apps::migration::engine::PathCache;
use crate::apps::migration::engine::SystemVars;
use crate::apps::migration::engine::TransformContext;
use crate::apps::migration::engine::TransformResult;
use crate::apps::migration::types::MatchStrategy;
use crate::lua::runtime::LuaRuntime;

// =============================================================================
// Types
// =============================================================================

/// Input for target matching.
pub struct MatchInput<'a> {
    /// The source record being matched.
    pub source_record: &'a Record,
    /// Match strategy (SameId, Find, or Lua).
    pub strategy: MatchStrategy,
    /// Source entity primary key field name.
    pub source_primary_key: &'a str,
    /// Target entity primary key field name.
    pub target_primary_key: &'a str,
    /// For Find strategy: (target_field, source chain) pairs.
    /// The source chain is executed against the source record to get the
    /// expected value, which is then compared against the target_field
    /// (possibly dotted) on each target record.
    pub match_conditions: &'a [(String, Vec<ChainItem>)],
    /// Source entity logical name.
    pub source_entity: &'a str,
    /// Target entity logical name.
    pub target_entity: &'a str,
    /// Find cache for resolving find() transforms within match condition chains.
    pub find_cache: &'a dyn FindCache,
    /// Pre-built Lua match index (source GUID → target GUID), if strategy is Lua.
    pub lua_match_index: Option<&'a LuaMatchIndex>,
}

/// Pre-built index mapping source record GUIDs to target record GUIDs.
/// Built once by running the Lua script, then used for O(1) per-record lookups.
pub type LuaMatchIndex = HashMap<Uuid, Uuid>;

/// Parsed result of calling `M.declare()` on a Lua match script.
#[derive(Debug, Clone, Default)]
pub struct LuaDeclare {
    /// Primary source entity logical name (entity-level Lua only).
    pub source: Option<String>,
    /// Primary target entity logical name (entity-level Lua only).
    pub target: Option<String>,
    /// Fields to fetch for the primary source entity.
    pub source_fields: Vec<String>,
    /// Fields to fetch for the primary target entity.
    pub target_fields: Vec<String>,
    /// Additional source entities to fetch: (entity_name, fields).
    pub source_entities: Vec<(String, Vec<String>)>,
    /// Additional target entities to fetch: (entity_name, fields).
    pub target_entities: Vec<(String, Vec<String>)>,
}

/// Pre-built index for O(1) target matching by ID.
///
/// Built once before the matching loop, maps target record UUIDs to their
/// index in the target records slice.
pub type TargetIndex = HashMap<Uuid, usize>;

/// Build a target index from target records for SameId matching.
///
/// Maps each target record's ID to its index in the slice.
/// Returns an error if any target record has no resolvable ID.
pub fn build_target_index(
    target_records: &[Record],
    primary_key: &str,
) -> Result<TargetIndex, TargetIndexError> {
    let mut index = HashMap::with_capacity(target_records.len());
    for (i, target) in target_records.iter().enumerate() {
        let id = target.id().or_else(|| match target.get(primary_key) {
            Some(Value::Guid(id)) => Some(*id),
            _ => None,
        });
        match id {
            Some(id) => {
                // Log specific record we're looking for + first few
                if id.to_string().contains("7cfac398") || i < 3 {
                    log::warn!(
                        "[matching] target_index[{}]: id={}, record.id()={:?}, field('{}')={:?}",
                        i,
                        id,
                        target.id(),
                        primary_key,
                        target.get(primary_key),
                    );
                }
                index.insert(id, i);
            }
            None => {
                return Err(TargetIndexError {
                    record_index: i,
                    primary_key: primary_key.to_string(),
                });
            }
        }
    }
    let search_id = Uuid::parse_str("7cfac398-9c0a-e511-80c2-005056a64738").ok();
    let found = search_id
        .as_ref()
        .map(|id| index.contains_key(id))
        .unwrap_or(false);
    log::warn!(
        "[matching] Built target index: {} entries, pk_field='{}', looking for 7cfac398-9c0a-e511-80c2-005056a64738? {}",
        index.len(),
        primary_key,
        found,
    );
    Ok(index)
}

/// Error when a target record cannot be indexed.
#[derive(Debug, Clone)]
pub struct TargetIndexError {
    /// Index of the problematic record in the target records slice.
    pub record_index: usize,
    /// The primary key field that was expected.
    pub primary_key: String,
}

impl std::fmt::Display for TargetIndexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Target record at index {} has no ID (expected primary key field '{}' as Guid)",
            self.record_index, self.primary_key,
        )
    }
}

/// Result of attempting to match a source record to a target record.
#[derive(Debug)]
pub enum MatchResult {
    /// Exactly one target record matched (index into target_records slice).
    Found(usize),
    /// No target record matched.
    NotFound,
    /// Multiple target records matched (ambiguous).
    Multiple(usize),
    /// An error occurred during matching.
    Error(String),
}

// =============================================================================
// Matching Logic
// =============================================================================

/// Match a source record to a target record.
///
/// Uses the strategy specified in `input`:
/// - **SameId**: O(1) lookup via pre-built `TargetIndex`.
/// - **Find**: Execute match condition chains, scan targets for matches.
pub fn match_target(
    input: &MatchInput<'_>,
    target_records: &[Record],
    target_index: &TargetIndex,
) -> MatchResult {
    match input.strategy {
        MatchStrategy::SameId => match_same_id(input, target_index),
        MatchStrategy::Find => match_find(input, target_records),
        MatchStrategy::Lua => match_lua(input, target_records, target_index),
    }
}

/// SameID matching: O(1) lookup via pre-built target index.
fn match_same_id(input: &MatchInput<'_>, target_index: &TargetIndex) -> MatchResult {
    // Get source record's primary key value
    let source_pk_value = match input.source_record.id() {
        Some(id) => id,
        None => {
            // Try getting from field name
            match input.source_record.get(input.source_primary_key) {
                Some(Value::Guid(id)) => *id,
                _ => {
                    return MatchResult::Error(format!(
                        "Source record missing primary key '{}'",
                        input.source_primary_key
                    ));
                }
            }
        }
    };

    // O(1) HashMap lookup
    match target_index.get(&source_pk_value) {
        Some(&idx) => MatchResult::Found(idx),
        None => {
            log::debug!(
                "[matching] SameId NotFound: source_pk={}, target_index_size={}, source_pk_field='{}', record.id()={:?}",
                source_pk_value,
                target_index.len(),
                input.source_primary_key,
                input.source_record.id(),
            );
            MatchResult::NotFound
        }
    }
}

/// Find-based matching: execute match condition chains, scan targets.
fn match_find(input: &MatchInput<'_>, target_records: &[Record]) -> MatchResult {
    // Step 1: Execute each match condition's source chain to get expected values
    let mut conditions: Vec<(&str, Value)> = Vec::new();

    let system_vars = SystemVars::new(
        Entity::logical(input.source_entity),
        Entity::logical(input.target_entity),
        0,
    );

    let empty_path_cache = PathCache::new();
    for (target_field, source_chain) in input.match_conditions {
        let mut ctx = TransformContext {
            source_record: input.source_record,
            variables: &std::collections::HashMap::new(),
            system_vars: system_vars.clone(),
            find_cache: input.find_cache,
            path_cache: &empty_path_cache,
        };

        match execute_chain(source_chain, &mut ctx) {
            TransformResult::Value(v) | TransformResult::Exit(v) => {
                conditions.push((target_field.as_str(), v));
            }
            TransformResult::Error(e) => {
                return MatchResult::Error(format!(
                    "Match condition chain for '{}' failed: {:?}",
                    target_field, e
                ));
            }
        }
    }

    // Step 2: Scan target records, checking all conditions
    let mut matches: Vec<usize> = Vec::new();

    for (i, target) in target_records.iter().enumerate() {
        let all_match =
            conditions
                .iter()
                .all(|(field, expected)| match traverse_path(target, field) {
                    Some(actual) => values_equal(actual, expected),
                    None => matches!(expected, Value::Null),
                });

        if all_match {
            matches.push(i);
        }
    }

    match matches.len() {
        0 => MatchResult::NotFound,
        1 => MatchResult::Found(matches[0]),
        n => MatchResult::Multiple(n),
    }
}

// =============================================================================
// Lua Matching
// =============================================================================

/// Lua matching: O(1) lookup via pre-built LuaMatchIndex.
fn match_lua(
    input: &MatchInput<'_>,
    target_records: &[Record],
    target_index: &TargetIndex,
) -> MatchResult {
    let lua_index = match input.lua_match_index {
        Some(idx) => idx,
        None => return MatchResult::Error("Lua match index not built".to_string()),
    };

    // Get source record's original primary key from the field value, not record.id().
    // record.id() may be a synthetic UUID (for junction entities) which won't match
    // the original GUID the Lua script used as its key.
    let source_pk = match input.source_record.get(input.source_primary_key) {
        Some(Value::Guid(id)) => *id,
        _ => match input.source_record.id() {
            Some(id) => id,
            None => {
                return MatchResult::Error(format!(
                    "Source record missing primary key '{}'",
                    input.source_primary_key
                ));
            }
        },
    };

    // Look up in Lua match index: source GUID → target GUID
    let target_guid = match lua_index.get(&source_pk) {
        Some(guid) => guid,
        None => return MatchResult::NotFound,
    };

    // Look up target GUID in target index: target GUID → target record index.
    // The target index may be keyed by synthetic IDs (for junction entities),
    // so also try matching by the original primary key field value.
    match target_index.get(target_guid) {
        Some(&idx) => MatchResult::Found(idx),
        None => {
            // Fallback: scan target records for one whose PK field matches.
            // The target index may be keyed by synthetic IDs (for junction entities),
            // so the original GUID from the Lua script won't be found directly.
            for (idx, record) in target_records.iter().enumerate() {
                if let Some(Value::Guid(pk)) = record.get(input.target_primary_key) {
                    if *pk == *target_guid {
                        return MatchResult::Found(idx);
                    }
                }
            }
            MatchResult::Error(format!(
                "Lua script returned target GUID {} but it's not in fetched target records",
                target_guid
            ))
        }
    }
}

/// Build a Lua match index by running the script once with all records.
///
/// The script's `M.resolve(source, target)` receives entity-keyed tables:
/// - `source.entity_name` → array of records
/// - `target.entity_name` → array of records
///
/// Returns `{ matches = { [source_guid] = target_guid } }`.
pub fn build_lua_match_index(
    script: &str,
    source_data: &HashMap<String, &[Record]>,
    target_data: &HashMap<String, &[Record]>,
) -> Result<LuaMatchIndex, String> {
    let runtime = LuaRuntime::new().map_err(|e| format!("Failed to create Lua runtime: {e}"))?;

    // Load the script module
    let module: Table = runtime
        .load(script)
        .map_err(|e| format!("Failed to load Lua script: {e}"))?;

    // Build source table: { entity_name = { record1, record2, ... }, ... }
    let source_table = build_entity_table(&runtime, source_data)?;
    let target_table = build_entity_table(&runtime, target_data)?;

    // Call M.resolve(source, target)
    let resolve: mlua::Function = module
        .get("resolve")
        .map_err(|e| format!("Script missing M.resolve(): {e}"))?;

    let result: Table = resolve
        .call((source_table, target_table))
        .map_err(|e| format!("M.resolve() failed: {e}"))?;

    // Check for error
    if let Ok(error_msg) = result.get::<mlua::String>("error") {
        let msg = error_msg.to_string_lossy();
        return Err(format!("Lua match error: {msg}"));
    }

    // Parse matches table: { [source_guid] = target_guid }
    let matches_table: Table = result
        .get("matches")
        .map_err(|e| format!("Result missing 'matches' field: {e}"))?;

    let mut index = HashMap::new();
    for pair in matches_table.pairs::<mlua::String, mlua::String>() {
        let (source_key, target_val) =
            pair.map_err(|e| format!("Invalid entry in matches table: {e}"))?;

        let source_str = source_key
            .to_str()
            .map_err(|e| format!("Invalid UTF-8 in source key: {e}"))?;
        let target_str = target_val
            .to_str()
            .map_err(|e| format!("Invalid UTF-8 in target value: {e}"))?;

        let source_uuid: Uuid = source_str
            .parse()
            .map_err(|e| format!("Invalid source GUID '{source_str}': {e}"))?;
        let target_uuid: Uuid = target_str
            .parse()
            .map_err(|e| format!("Invalid target GUID '{target_str}': {e}"))?;

        index.insert(source_uuid, target_uuid);
    }

    log::info!("[matching] Lua match index built: {} entries", index.len());

    Ok(index)
}

/// Build a Lua table keyed by entity name, each containing an array of records.
pub(crate) fn build_entity_table(
    runtime: &LuaRuntime,
    data: &HashMap<String, &[Record]>,
) -> Result<Table, String> {
    let table = runtime
        .create_table()
        .map_err(|e| format!("Failed to create table: {e}"))?;

    for (entity_name, records) in data {
        let entity_table = runtime
            .create_table()
            .map_err(|e| format!("Failed to create table for {entity_name}: {e}"))?;

        for (i, record) in records.iter().enumerate() {
            let record_json = serde_json::to_value(record)
                .map_err(|e| format!("Failed to serialize record: {e}"))?;
            let record_lua = runtime
                .json_to_lua(&record_json)
                .map_err(|e| format!("Failed to convert record to Lua: {e}"))?;
            entity_table
                .set(i + 1, record_lua)
                .map_err(|e| format!("Failed to insert record: {e}"))?;
        }

        table
            .set(entity_name.as_str(), entity_table)
            .map_err(|e| format!("Failed to set entity {entity_name}: {e}"))?;
    }

    Ok(table)
}

/// Parse the `M.declare()` function from a Lua match script.
///
/// Returns the declared source and target entities with their fields.
/// If `M.declare()` doesn't exist, returns an empty declare (only the
/// mapping's own entities will be used).
pub fn parse_lua_declare(script: &str) -> Result<LuaDeclare, String> {
    let runtime = LuaRuntime::new().map_err(|e| format!("Failed to create Lua runtime: {e}"))?;

    let module: Table = runtime
        .load(script)
        .map_err(|e| format!("Failed to load Lua script: {e}"))?;

    // M.declare() is optional
    let declare_fn: mlua::Function = match module.get("declare") {
        Ok(f) => f,
        Err(_) => return Ok(LuaDeclare::default()),
    };

    let result: Table = declare_fn
        .call(())
        .map_err(|e| format!("M.declare() failed: {e}"))?;

    let mut declare = LuaDeclare::default();

    // Parse "source" — three formats:
    //   1. String: just entity name (backward compat, no field list)
    //   2. Table with "entity" key: { entity = "name", fields = { ... } }
    //   3. Table without "entity" key: legacy extra-entities map
    match result.get::<mlua::Value>("source") {
        Ok(mlua::Value::String(s)) => {
            declare.source = Some(
                s.to_str()
                    .map_err(|e| format!("Invalid UTF-8 in source entity: {e}"))?
                    .to_string(),
            );
        }
        Ok(mlua::Value::Table(table)) => {
            // New format: table with entity + fields
            if let Ok(entity_str) = table.get::<mlua::String>("entity") {
                declare.source = Some(
                    entity_str
                        .to_str()
                        .map_err(|e| format!("Invalid UTF-8 in source entity: {e}"))?
                        .to_string(),
                );
                declare.source_fields = parse_fields_list(&table)?;
            } else {
                // Legacy format: source is a table of extra entity declarations
                declare.source_entities = parse_entity_declarations(&table)?;
            }
        }
        _ => {}
    }

    // Parse "target" — same three formats as source
    match result.get::<mlua::Value>("target") {
        Ok(mlua::Value::String(s)) => {
            declare.target = Some(
                s.to_str()
                    .map_err(|e| format!("Invalid UTF-8 in target entity: {e}"))?
                    .to_string(),
            );
        }
        Ok(mlua::Value::Table(table)) => {
            if let Ok(entity_str) = table.get::<mlua::String>("entity") {
                declare.target = Some(
                    entity_str
                        .to_str()
                        .map_err(|e| format!("Invalid UTF-8 in target entity: {e}"))?
                        .to_string(),
                );
                declare.target_fields = parse_fields_list(&table)?;
            } else {
                // Legacy format: target is a table of extra entity declarations
                declare.target_entities = parse_entity_declarations(&table)?;
            }
        }
        _ => {}
    }

    // Parse "source_entities" — extra source entities (new format, used alongside string source/target)
    if let Ok(source_entities_table) = result.get::<Table>("source_entities") {
        declare
            .source_entities
            .extend(parse_entity_declarations(&source_entities_table)?);
    }

    // Parse "target_entities" — extra target entities (new format, used alongside string source/target)
    if let Ok(target_entities_table) = result.get::<Table>("target_entities") {
        declare
            .target_entities
            .extend(parse_entity_declarations(&target_entities_table)?);
    }

    Ok(declare)
}

/// Parse entity declarations from a Lua table: { entity_name = { fields = { ... } } }
fn parse_entity_declarations(table: &Table) -> Result<Vec<(String, Vec<String>)>, String> {
    let mut entities = Vec::new();

    for pair in table.pairs::<mlua::String, Table>() {
        let (key, value) = pair.map_err(|e| format!("Invalid entity declaration: {e}"))?;
        let entity_name = key
            .to_str()
            .map_err(|e| format!("Invalid UTF-8 in entity name: {e}"))?
            .to_string();

        let mut fields = Vec::new();
        if let Ok(fields_table) = value.get::<Table>("fields") {
            for field_pair in fields_table.pairs::<i64, mlua::String>() {
                let (_, field_val) =
                    field_pair.map_err(|e| format!("Invalid field in {entity_name}: {e}"))?;
                let field_str = field_val
                    .to_str()
                    .map_err(|e| format!("Invalid UTF-8 in field: {e}"))?
                    .to_string();
                fields.push(field_str);
            }
        }

        entities.push((entity_name, fields));
    }

    Ok(entities)
}

/// Parse a `fields` array from a Lua table: { fields = { "f1", "f2", ... } }
fn parse_fields_list(table: &Table) -> Result<Vec<String>, String> {
    let mut fields = Vec::new();
    if let Ok(fields_table) = table.get::<Table>("fields") {
        for field_pair in fields_table.pairs::<i64, mlua::String>() {
            let (_, field_val) =
                field_pair.map_err(|e| format!("Invalid field in fields list: {e}"))?;
            let field_str = field_val
                .to_str()
                .map_err(|e| format!("Invalid UTF-8 in field: {e}"))?
                .to_string();
            fields.push(field_str);
        }
    }
    Ok(fields)
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::apps::migration::engine::StubFindCache;
    use crate::apps::migration::types::TransformData;
    use uuid::Uuid;

    fn id(n: u128) -> Uuid {
        Uuid::from_u128(n)
    }

    fn make_record(entity: &str, uuid: Uuid, fields: Vec<(&str, Value)>) -> Record {
        let mut record = Record::with_id(Entity::logical(entity), uuid);
        for (field, value) in fields {
            record = record.set(field, value);
        }
        record
    }

    fn default_input<'a>(
        source: &'a Record,
        strategy: MatchStrategy,
        conditions: &'a [(String, Vec<ChainItem>)],
    ) -> MatchInput<'a> {
        static STUB: StubFindCache = StubFindCache;
        MatchInput {
            source_record: source,
            strategy,
            source_primary_key: "accountid",
            target_primary_key: "accountid",
            match_conditions: conditions,
            source_entity: "account",
            target_entity: "account",
            find_cache: &STUB,
            lua_match_index: None,
        }
    }

    // ---- SameID tests ----

    #[test]
    fn same_id_match() {
        let source = make_record("account", id(1), vec![("name", Value::from("Acme"))]);
        let targets = vec![
            make_record("account", id(2), vec![("name", Value::from("Other"))]),
            make_record("account", id(1), vec![("name", Value::from("Acme Target"))]),
        ];

        let index = build_target_index(&targets, "accountid").unwrap();
        let input = default_input(&source, MatchStrategy::SameId, &[]);
        let result = match_target(&input, &targets, &index);

        assert!(matches!(result, MatchResult::Found(idx) if targets[idx].id() == Some(id(1))));
    }

    #[test]
    fn same_id_no_match() {
        let source = make_record("account", id(1), vec![]);
        let targets = vec![
            make_record("account", id(2), vec![]),
            make_record("account", id(3), vec![]),
        ];

        let index = build_target_index(&targets, "accountid").unwrap();
        let input = default_input(&source, MatchStrategy::SameId, &[]);
        let result = match_target(&input, &targets, &index);

        assert!(matches!(result, MatchResult::NotFound));
    }

    #[test]
    fn same_id_empty_targets() {
        let source = make_record("account", id(1), vec![]);
        let index = build_target_index(&[], "accountid").unwrap();
        let input = default_input(&source, MatchStrategy::SameId, &[]);
        let result = match_target(&input, &[], &index);

        assert!(matches!(result, MatchResult::NotFound));
    }

    // ---- Find tests ----

    #[test]
    fn find_single_condition_match() {
        let source = make_record("account", id(1), vec![("name", Value::from("Acme"))]);
        let targets = vec![
            make_record("account", id(10), vec![("name", Value::from("Other"))]),
            make_record("account", id(11), vec![("name", Value::from("Acme"))]),
        ];

        let conditions = vec![(
            "name".to_string(),
            vec![ChainItem::new(TransformData::Copy {
                path: "name".to_string(),
            })],
        )];

        let index = build_target_index(&targets, "accountid").unwrap();
        let input = default_input(&source, MatchStrategy::Find, &conditions);
        let result = match_target(&input, &targets, &index);

        assert!(matches!(result, MatchResult::Found(idx) if targets[idx].id() == Some(id(11))));
    }

    #[test]
    fn find_multiple_conditions() {
        let source = make_record(
            "account",
            id(1),
            vec![("name", Value::from("Acme")), ("city", Value::from("NYC"))],
        );
        let targets = vec![
            make_record(
                "account",
                id(10),
                vec![("name", Value::from("Acme")), ("city", Value::from("LA"))],
            ),
            make_record(
                "account",
                id(11),
                vec![("name", Value::from("Acme")), ("city", Value::from("NYC"))],
            ),
        ];

        let conditions = vec![
            (
                "name".to_string(),
                vec![ChainItem::new(TransformData::Copy {
                    path: "name".to_string(),
                })],
            ),
            (
                "city".to_string(),
                vec![ChainItem::new(TransformData::Copy {
                    path: "city".to_string(),
                })],
            ),
        ];

        let index = build_target_index(&targets, "accountid").unwrap();
        let input = default_input(&source, MatchStrategy::Find, &conditions);
        let result = match_target(&input, &targets, &index);

        assert!(matches!(result, MatchResult::Found(idx) if targets[idx].id() == Some(id(11))));
    }

    #[test]
    fn find_no_match() {
        let source = make_record("account", id(1), vec![("name", Value::from("Acme"))]);
        let targets = vec![
            make_record("account", id(10), vec![("name", Value::from("Other"))]),
            make_record("account", id(11), vec![("name", Value::from("Another"))]),
        ];

        let conditions = vec![(
            "name".to_string(),
            vec![ChainItem::new(TransformData::Copy {
                path: "name".to_string(),
            })],
        )];

        let index = build_target_index(&targets, "accountid").unwrap();
        let input = default_input(&source, MatchStrategy::Find, &conditions);
        let result = match_target(&input, &targets, &index);

        assert!(matches!(result, MatchResult::NotFound));
    }

    #[test]
    fn find_multiple_matches() {
        let source = make_record("account", id(1), vec![("name", Value::from("Acme"))]);
        let targets = vec![
            make_record("account", id(10), vec![("name", Value::from("Acme"))]),
            make_record("account", id(11), vec![("name", Value::from("Acme"))]),
        ];

        let conditions = vec![(
            "name".to_string(),
            vec![ChainItem::new(TransformData::Copy {
                path: "name".to_string(),
            })],
        )];

        let index = build_target_index(&targets, "accountid").unwrap();
        let input = default_input(&source, MatchStrategy::Find, &conditions);
        let result = match_target(&input, &targets, &index);

        assert!(matches!(result, MatchResult::Multiple(2)));
    }

    #[test]
    fn find_with_dotted_target_field() {
        // Target records have nested contact with email — match on dotted path
        let nested1 = Record::with_id(Entity::logical("contact"), id(100))
            .set("emailaddress1", Value::from("other@example.com"));
        let nested2 = Record::with_id(Entity::logical("contact"), id(101))
            .set("emailaddress1", Value::from("alice@example.com"));

        let source = make_record(
            "account",
            id(1),
            vec![("email", Value::from("alice@example.com"))],
        );
        let targets = vec![
            make_record(
                "account",
                id(10),
                vec![("primarycontactid", Value::Record(Arc::new(nested1)))],
            ),
            make_record(
                "account",
                id(11),
                vec![("primarycontactid", Value::Record(Arc::new(nested2)))],
            ),
        ];

        let conditions = vec![(
            "primarycontactid.emailaddress1".to_string(),
            vec![ChainItem::new(TransformData::Copy {
                path: "email".to_string(),
            })],
        )];

        let index = build_target_index(&targets, "accountid").unwrap();
        let input = default_input(&source, MatchStrategy::Find, &conditions);
        let result = match_target(&input, &targets, &index);

        assert!(matches!(result, MatchResult::Found(idx) if targets[idx].id() == Some(id(11))));
    }

    #[test]
    fn find_with_constant_chain() {
        // Source chain produces a constant instead of copying from source
        let source = make_record("account", id(1), vec![]);
        let targets = vec![
            make_record("account", id(10), vec![("status", Value::Int(0))]),
            make_record("account", id(11), vec![("status", Value::Int(1))]),
        ];

        let conditions = vec![(
            "status".to_string(),
            vec![ChainItem::new(TransformData::Constant {
                value: Value::Int(1),
            })],
        )];

        let index = build_target_index(&targets, "accountid").unwrap();
        let input = default_input(&source, MatchStrategy::Find, &conditions);
        let result = match_target(&input, &targets, &index);

        assert!(matches!(result, MatchResult::Found(idx) if targets[idx].id() == Some(id(11))));
    }

    #[test]
    fn find_chain_error_returns_error() {
        // Source chain references a missing field → error
        let source = make_record("account", id(1), vec![]);

        let conditions = vec![(
            "name".to_string(),
            vec![ChainItem::new(TransformData::Copy {
                path: "nonexistent".to_string(),
            })],
        )];

        let index = build_target_index(&[], "accountid").unwrap();
        let input = default_input(&source, MatchStrategy::Find, &conditions);
        let result = match_target(&input, &[], &index);

        assert!(matches!(result, MatchResult::Error(_)));
    }

    #[test]
    fn find_empty_targets() {
        let source = make_record("account", id(1), vec![("name", Value::from("Acme"))]);

        let conditions = vec![(
            "name".to_string(),
            vec![ChainItem::new(TransformData::Copy {
                path: "name".to_string(),
            })],
        )];

        let index = build_target_index(&[], "accountid").unwrap();
        let input = default_input(&source, MatchStrategy::Find, &conditions);
        let result = match_target(&input, &[], &index);

        assert!(matches!(result, MatchResult::NotFound));
    }

    // ---- Lua tests ----

    #[test]
    fn lua_match_via_index() {
        let source = make_record("account", id(1), vec![("name", Value::from("Acme"))]);
        let targets = vec![
            make_record("account", id(10), vec![("name", Value::from("Other"))]),
            make_record(
                "account",
                id(11),
                vec![("name", Value::from("Acme Target"))],
            ),
        ];

        let mut lua_index = LuaMatchIndex::new();
        lua_index.insert(id(1), id(11));

        let target_index = build_target_index(&targets, "accountid").unwrap();
        let mut input = default_input(&source, MatchStrategy::Lua, &[]);
        input.lua_match_index = Some(&lua_index);
        let result = match_target(&input, &targets, &target_index);

        assert!(matches!(result, MatchResult::Found(idx) if targets[idx].id() == Some(id(11))));
    }

    #[test]
    fn lua_match_not_found_in_index() {
        let source = make_record("account", id(1), vec![]);
        let targets = vec![make_record("account", id(10), vec![])];

        let lua_index = LuaMatchIndex::new(); // empty

        let target_index = build_target_index(&targets, "accountid").unwrap();
        let mut input = default_input(&source, MatchStrategy::Lua, &[]);
        input.lua_match_index = Some(&lua_index);
        let result = match_target(&input, &targets, &target_index);

        assert!(matches!(result, MatchResult::NotFound));
    }

    #[test]
    fn lua_match_missing_index_errors() {
        let source = make_record("account", id(1), vec![]);
        let targets = vec![make_record("account", id(10), vec![])];

        let target_index = build_target_index(&targets, "accountid").unwrap();
        let input = default_input(&source, MatchStrategy::Lua, &[]); // lua_match_index = None
        let result = match_target(&input, &targets, &target_index);

        assert!(matches!(result, MatchResult::Error(_)));
    }

    #[test]
    fn lua_match_target_guid_not_in_fetched_errors() {
        let source = make_record("account", id(1), vec![]);
        let targets = vec![make_record("account", id(10), vec![])];

        let mut lua_index = LuaMatchIndex::new();
        lua_index.insert(id(1), id(99)); // target 99 doesn't exist in fetched targets

        let target_index = build_target_index(&targets, "accountid").unwrap();
        let mut input = default_input(&source, MatchStrategy::Lua, &[]);
        input.lua_match_index = Some(&lua_index);
        let result = match_target(&input, &targets, &target_index);

        assert!(matches!(result, MatchResult::Error(_)));
    }

    #[test]
    fn build_lua_match_index_simple_script() {
        let source_records = vec![
            make_record(
                "cgk_support",
                id(1),
                vec![
                    (
                        "cgk_supportid",
                        Value::from("00000000-0000-0000-0000-000000000001"),
                    ),
                    ("cgk_name", Value::from("Alpha")),
                ],
            ),
            make_record(
                "cgk_support",
                id(2),
                vec![
                    (
                        "cgk_supportid",
                        Value::from("00000000-0000-0000-0000-000000000002"),
                    ),
                    ("cgk_name", Value::from("Beta")),
                ],
            ),
        ];

        let target_records = vec![
            make_record(
                "nrq_support",
                id(10),
                vec![
                    (
                        "nrq_supportid",
                        Value::from("00000000-0000-0000-0000-00000000000a"),
                    ),
                    ("nrq_name", Value::from("Beta")),
                ],
            ),
            make_record(
                "nrq_support",
                id(11),
                vec![
                    (
                        "nrq_supportid",
                        Value::from("00000000-0000-0000-0000-00000000000b"),
                    ),
                    ("nrq_name", Value::from("Alpha")),
                ],
            ),
        ];

        let script = r#"
local M = {}
function M.resolve(source, target)
    local matches = {}
    for _, s in ipairs(source.cgk_support) do
        for _, t in ipairs(target.nrq_support) do
            if s.cgk_name == t.nrq_name then
                matches[s.cgk_supportid] = t.nrq_supportid
            end
        end
    end
    return { matches = matches }
end
return M
"#;

        let mut source_data = HashMap::new();
        source_data.insert("cgk_support".to_string(), source_records.as_slice());
        let mut target_data = HashMap::new();
        target_data.insert("nrq_support".to_string(), target_records.as_slice());

        let index = build_lua_match_index(script, &source_data, &target_data).unwrap();

        assert_eq!(index.len(), 2);
        // Alpha(id=1) → Alpha target (id=00..0b)
        assert_eq!(
            index.get(&Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap()),
            Some(&Uuid::parse_str("00000000-0000-0000-0000-00000000000b").unwrap()),
        );
        // Beta(id=2) → Beta target (id=00..0a)
        assert_eq!(
            index.get(&Uuid::parse_str("00000000-0000-0000-0000-000000000002").unwrap()),
            Some(&Uuid::parse_str("00000000-0000-0000-0000-00000000000a").unwrap()),
        );
    }

    #[test]
    fn build_lua_match_index_error_from_script() {
        let script = r#"
local M = {}
function M.resolve(source, target)
    return { error = "something went wrong" }
end
return M
"#;
        let source_data = HashMap::new();
        let target_data = HashMap::new();

        let result = build_lua_match_index(script, &source_data, &target_data);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("something went wrong"));
    }

    #[test]
    fn build_lua_match_index_missing_resolve() {
        let script = r#"
local M = {}
return M
"#;
        let source_data = HashMap::new();
        let target_data = HashMap::new();

        let result = build_lua_match_index(script, &source_data, &target_data);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("M.resolve()"));
    }

    #[test]
    fn parse_lua_declare_legacy_table_format() {
        // Legacy format: source/target are tables of extra entity declarations
        let script = r#"
local M = {}
function M.declare()
    return {
        source = {
            cgk_support = { fields = {"cgk_supportid", "cgk_name"} },
        },
        target = {
            nrq_support = { fields = {"nrq_supportid", "nrq_name"} },
        },
    }
end
function M.resolve(source, target)
    return { matches = {} }
end
return M
"#;

        let declare = parse_lua_declare(script).unwrap();
        assert!(declare.source.is_none());
        assert!(declare.target.is_none());
        assert_eq!(declare.source_entities.len(), 1);
        assert_eq!(declare.source_entities[0].0, "cgk_support");
        assert_eq!(
            declare.source_entities[0].1,
            vec!["cgk_supportid", "cgk_name"]
        );
        assert_eq!(declare.target_entities.len(), 1);
        assert_eq!(declare.target_entities[0].0, "nrq_support");
        assert_eq!(
            declare.target_entities[0].1,
            vec!["nrq_supportid", "nrq_name"]
        );
    }

    #[test]
    fn parse_lua_declare_with_primary_entities() {
        // New format: source/target are strings, extras in source_entities/target_entities
        let script = r#"
local M = {}
function M.declare()
    return {
        source = "cgk_account",
        target = "nrq_account",
        source_entities = {
            cgk_contact = { fields = {"fullname"} },
        },
        target_entities = {
            nrq_contact = { fields = {"fullname"} },
        },
    }
end
function M.resolve(source, target)
    return { results = {} }
end
return M
"#;

        let declare = parse_lua_declare(script).unwrap();
        assert_eq!(declare.source.as_deref(), Some("cgk_account"));
        assert_eq!(declare.target.as_deref(), Some("nrq_account"));
        assert_eq!(declare.source_entities.len(), 1);
        assert_eq!(declare.source_entities[0].0, "cgk_contact");
        assert_eq!(declare.source_entities[0].1, vec!["fullname"]);
        assert_eq!(declare.target_entities.len(), 1);
        assert_eq!(declare.target_entities[0].0, "nrq_contact");
        assert_eq!(declare.target_entities[0].1, vec!["fullname"]);
    }

    #[test]
    fn parse_lua_declare_primary_entities_no_extras() {
        let script = r#"
local M = {}
function M.declare()
    return {
        source = "cgk_account",
        target = "nrq_account",
    }
end
function M.resolve(source, target)
    return { results = {} }
end
return M
"#;

        let declare = parse_lua_declare(script).unwrap();
        assert_eq!(declare.source.as_deref(), Some("cgk_account"));
        assert_eq!(declare.target.as_deref(), Some("nrq_account"));
        assert!(declare.source_fields.is_empty());
        assert!(declare.target_fields.is_empty());
        assert!(declare.source_entities.is_empty());
        assert!(declare.target_entities.is_empty());
    }

    #[test]
    fn parse_lua_declare_table_with_entity_and_fields() {
        // New format: source/target are tables with entity + fields
        let script = r#"
local M = {}
function M.declare()
    return {
        source = { entity = "cgk_folder", fields = {"cgk_folderid", "cgk_name", "cgk_foldernumber"} },
        target = { entity = "nrq_project", fields = {"nrq_projectid", "nrq_dossiernummerguid"} },
        source_entities = {
            cgk_film = { fields = {"cgk_filmid"} },
        },
    }
end
function M.resolve(source, target)
    return { results = {} }
end
return M
"#;

        let declare = parse_lua_declare(script).unwrap();
        assert_eq!(declare.source.as_deref(), Some("cgk_folder"));
        assert_eq!(declare.target.as_deref(), Some("nrq_project"));
        assert_eq!(
            declare.source_fields,
            vec!["cgk_folderid", "cgk_name", "cgk_foldernumber"]
        );
        assert_eq!(
            declare.target_fields,
            vec!["nrq_projectid", "nrq_dossiernummerguid"]
        );
        assert_eq!(declare.source_entities.len(), 1);
        assert_eq!(declare.source_entities[0].0, "cgk_film");
    }

    #[test]
    fn parse_lua_declare_without_declare_fn() {
        let script = r#"
local M = {}
function M.resolve(source, target)
    return { matches = {} }
end
return M
"#;

        let declare = parse_lua_declare(script).unwrap();
        assert!(declare.source.is_none());
        assert!(declare.target.is_none());
        assert!(declare.source_entities.is_empty());
        assert!(declare.target_entities.is_empty());
    }

    #[test]
    fn build_lua_match_index_with_lib_find() {
        // Test that lib.find() is available in the Lua runtime
        let source_records = vec![make_record(
            "src",
            id(1),
            vec![
                ("srcid", Value::from("00000000-0000-0000-0000-000000000001")),
                ("name", Value::from("Test")),
            ],
        )];

        let target_records = vec![make_record(
            "tgt",
            id(10),
            vec![
                ("tgtid", Value::from("00000000-0000-0000-0000-00000000000a")),
                ("name", Value::from("Test")),
            ],
        )];

        let script = r#"
local M = {}
function M.resolve(source, target)
    local matches = {}
    for _, s in ipairs(source.src) do
        local match = lib.find(target.tgt, "name", s.name)
        if match then
            matches[s.srcid] = match.tgtid
        end
    end
    return { matches = matches }
end
return M
"#;

        let mut source_data = HashMap::new();
        source_data.insert("src".to_string(), source_records.as_slice());
        let mut target_data = HashMap::new();
        target_data.insert("tgt".to_string(), target_records.as_slice());

        let index = build_lua_match_index(script, &source_data, &target_data).unwrap();

        assert_eq!(index.len(), 1);
        assert_eq!(
            index.get(&Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap()),
            Some(&Uuid::parse_str("00000000-0000-0000-0000-00000000000a").unwrap()),
        );
    }
}
