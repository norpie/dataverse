//! Entity-level Lua execution — replaces transforms AND matching via a single script.
//!
//! The script's `M.resolve(source, target)` receives entity-keyed tables and returns
//! per-source-record results: desired field values and optional target GUID matches.

use std::collections::HashMap;
use std::collections::HashSet;

use chrono::DateTime;
use dataverse_lib::model::Entity;
use dataverse_lib::model::Record;
use dataverse_lib::model::Value;
use dataverse_lib::model::types::EntityBinding;
use dataverse_lib::model::types::EntityReference;
use mlua::Table;
use uuid::Uuid;

use super::OperationType;
use super::RecordComparison;
use super::diff::diff_fields;
use super::matching::LuaMatchIndex;
use super::matching::build_entity_table;
use crate::apps::migration::engine::TransformError;
use crate::apps::migration::engine::record::RecordResult;
use crate::lua::runtime::LuaRuntime;

// =============================================================================
// Public API
// =============================================================================

/// An independent record to create/update, not tied to any source record.
#[derive(Debug, Clone)]
pub struct LuaCreateEntry {
    /// The desired target record primary key (e.g., a deterministic GUID).
    pub id: Uuid,
    /// Field values for the target record.
    pub fields: HashMap<String, Value>,
}

/// An Excel export produced by a Lua script.
#[derive(Debug, Clone)]
pub struct LuaExport {
    /// Sheet/file name (e.g., "impulspremies").
    pub name: String,
    /// Column headers.
    pub headers: Vec<String>,
    /// Data rows (each row is a vector of string cell values).
    pub rows: Vec<Vec<String>>,
}

/// Result of executing an entity-level Lua script.
#[derive(Debug)]
pub struct EntityLuaResult {
    /// Per-record transform results (same order as source records).
    pub record_results: Vec<RecordResult>,
    /// Source GUID → target GUID mapping (for comparison engine).
    pub match_index: LuaMatchIndex,
    /// Independent creates — results keyed by IDs not in source records.
    pub creates: Vec<LuaCreateEntry>,
    /// Excel exports to write to disk.
    pub exports: Vec<LuaExport>,
}

/// Execute an entity-level Lua script against source/target records.
///
/// 1. Creates a sandboxed Lua runtime and loads the script.
/// 2. Builds entity-keyed tables from all source/target records (primary + extras).
/// 3. Calls `M.resolve(source, target)` to get per-record results.
/// 4. Parses the results into `RecordResult`s and a `LuaMatchIndex`.
pub fn execute_entity_lua(
    script: &str,
    source_records: &[Record],
    source_entity: &str,
    target_entity: &str,
    extra_source: &[(String, Vec<Record>)],
    extra_target: &[(String, Vec<Record>)],
) -> Result<EntityLuaResult, String> {
    let runtime = LuaRuntime::new().map_err(|e| format!("Failed to create Lua runtime: {e}"))?;

    // Load the script module
    let module: Table = runtime
        .load(script)
        .map_err(|e| format!("Failed to load Lua script: {e}"))?;

    // Build source data map: primary entity + extras
    let mut source_data: HashMap<String, &[Record]> = HashMap::new();
    source_data.insert(source_entity.to_string(), source_records);
    for (entity, records) in extra_source {
        source_data.insert(entity.clone(), records.as_slice());
    }

    // Build target data map: primary entity is empty (will be filled by caller via target_records
    // passed as extra_target or as primary), plus extras
    // Note: target primary records come in as extra_target with the target_entity key
    let empty_target: Vec<Record> = vec![];
    let mut target_data: HashMap<String, &[Record]> = HashMap::new();
    // Default to empty for primary target entity (overridden if in extra_target)
    target_data.insert(target_entity.to_string(), &empty_target);
    for (entity, records) in extra_target {
        target_data.insert(entity.clone(), records.as_slice());
    }

    let source_table = build_entity_table(&runtime, &source_data)?;
    let target_table = build_entity_table(&runtime, &target_data)?;

    // Call M.resolve(source, target)
    let resolve: mlua::Function = module
        .get("resolve")
        .map_err(|e| format!("Script missing M.resolve(): {e}"))?;

    let result: Table = resolve
        .call((source_table, target_table))
        .map_err(|e| format!("M.resolve() failed: {e}"))?;

    // Check for global error
    if let Ok(error_msg) = result.get::<mlua::String>("error") {
        let msg = error_msg.to_string_lossy();
        return Err(format!("Lua script error: {msg}"));
    }

    // Parse results table
    let results_table: Table = result
        .get("results")
        .map_err(|e| format!("Result missing 'results' field: {e}"))?;

    // Build set of source record IDs for detecting independent creates
    let source_id_set: HashSet<String> = source_records
        .iter()
        .filter_map(|r| r.id().map(|id| id.to_string()))
        .collect();

    // Build results for each source record (in order)
    let mut record_results = Vec::with_capacity(source_records.len());
    let mut match_index = LuaMatchIndex::new();

    for source_record in source_records {
        let source_id = source_record_id(source_record);

        let Some(source_id) = source_id else {
            record_results.push(RecordResult {
                fields: HashMap::new(),
                errors: vec![(
                    "_script".to_string(),
                    TransformError::other("Source record has no ID"),
                )],
                skipped: false,
            });
            continue;
        };

        let source_id_str = source_id.to_string();

        // Look up this record in the results table
        let entry: mlua::Value = results_table
            .get(source_id_str.as_str())
            .map_err(|e| format!("Failed to read results for {source_id_str}: {e}"))?;

        match entry {
            mlua::Value::Table(entry_table) => {
                // Check for per-record error
                if let Ok(error_msg) = entry_table.get::<mlua::String>("error") {
                    let msg = error_msg.to_string_lossy();
                    record_results.push(RecordResult {
                        fields: HashMap::new(),
                        errors: vec![("_script".to_string(), TransformError::other(msg))],
                        skipped: false,
                    });
                    continue;
                }

                // Extract target GUID (optional — nil means create)
                match entry_table.get::<mlua::Value>("target") {
                    Ok(mlua::Value::String(s)) => {
                        let target_str = s
                            .to_str()
                            .map_err(|e| format!("Invalid UTF-8 in target GUID: {e}"))?;
                        let target_uuid: Uuid = target_str
                            .parse()
                            .map_err(|e| format!("Invalid target GUID '{target_str}': {e}"))?;
                        match_index.insert(source_id, target_uuid);
                    }
                    Ok(mlua::Value::Nil) => {
                        // No match — record will be created
                    }
                    Ok(other) => {
                        return Err(format!(
                            "Expected string or nil for 'target' field on record {source_id_str}, got {:?}",
                            other.type_name()
                        ));
                    }
                    Err(_) => {
                        // No target key at all — treat as create
                    }
                }

                // Extract fields table
                let fields = match entry_table.get::<Table>("fields") {
                    Ok(fields_table) => parse_fields_table(&runtime, &fields_table)?,
                    Err(_) => HashMap::new(),
                };

                record_results.push(RecordResult {
                    fields,
                    errors: vec![],
                    skipped: false,
                });
            }
            mlua::Value::Nil => {
                // Record not in results — script intentionally skipped it.
                // Mark as skipped so the comparison engine emits IgnoreSource
                // without consulting matching or NoMatchFallback.
                record_results.push(RecordResult {
                    fields: HashMap::new(),
                    errors: vec![],
                    skipped: true,
                });
            }
            other => {
                return Err(format!(
                    "Expected table or nil for record {source_id_str}, got {:?}",
                    other.type_name()
                ));
            }
        }
    }

    // Collect independent creates: results table keys that are NOT source record IDs.
    let mut creates = Vec::new();
    for pair in results_table.pairs::<mlua::String, mlua::Value>() {
        let (key, value) = pair.map_err(|e| format!("Failed to iterate results table: {e}"))?;
        let key_str = key
            .to_str()
            .map_err(|e| format!("Invalid UTF-8 in results key: {e}"))?;

        // Skip source-record-keyed entries (already processed above)
        if source_id_set.contains(&key_str.to_string()) {
            continue;
        }

        // Parse the key as a UUID
        let create_id: Uuid = key_str
            .parse()
            .map_err(|e| format!("Invalid GUID key '{key_str}' in results table: {e}"))?;

        match value {
            mlua::Value::Table(entry_table) => {
                // Check for per-record error
                if let Ok(error_msg) = entry_table.get::<mlua::String>("error") {
                    let msg = error_msg.to_string_lossy();
                    log::warn!("[entity_lua] Independent create {key_str} has error: {msg}");
                    continue;
                }

                // Extract fields table
                let fields = match entry_table.get::<Table>("fields") {
                    Ok(fields_table) => parse_fields_table(&runtime, &fields_table)?,
                    Err(_) => HashMap::new(),
                };

                creates.push(LuaCreateEntry {
                    id: create_id,
                    fields,
                });
            }
            mlua::Value::Nil => {
                // Explicitly nil — skip
            }
            other => {
                return Err(format!(
                    "Expected table or nil for independent create {key_str}, got {:?}",
                    other.type_name()
                ));
            }
        }
    }

    // Parse optional exports table
    let exports = match result.get::<Table>("exports") {
        Ok(exports_table) => parse_exports_table(&exports_table)?,
        Err(_) => Vec::new(),
    };

    log::info!(
        "[entity_lua] Executed: {} record results, {} match entries, {} independent creates, {} exports",
        record_results.len(),
        match_index.len(),
        creates.len(),
        exports.len(),
    );

    Ok(EntityLuaResult {
        record_results,
        match_index,
        creates,
        exports,
    })
}

// =============================================================================
// Independent Creates Processing
// =============================================================================

/// Process independent creates from a Lua entity script.
///
/// For each `LuaCreateEntry`:
/// - If the ID exists in `target_records` → diff fields → `Update` or `Skip`
/// - If not → `Create`
///
/// Returns the comparison entries and the set of target IDs that were matched
/// (so the caller can remove them from orphan detection).
pub fn process_lua_creates(
    creates: Vec<LuaCreateEntry>,
    target_records: &[Record],
    target_primary_key: &str,
) -> (Vec<RecordComparison>, HashSet<Uuid>) {
    // Build target index: GUID → index into target_records
    let target_index: HashMap<Uuid, usize> = target_records
        .iter()
        .enumerate()
        .filter_map(|(i, r)| {
            let id = r.id().or_else(|| match r.get(target_primary_key) {
                Some(Value::Guid(id)) => Some(*id),
                _ => None,
            });
            id.map(|id| (id, i))
        })
        .collect();

    let mut comparisons = Vec::with_capacity(creates.len());
    let mut matched_ids = HashSet::new();

    for create in creates {
        if let Some(&target_idx) = target_index.get(&create.id) {
            // Target exists — diff to determine Update or Skip
            let target = &target_records[target_idx];
            let target_id = target.id();
            if let Some(tid) = target_id {
                matched_ids.insert(tid);
            }

            let target_statecode = target.get("statecode").cloned();
            let target_statuscode = target.get("statuscode").cloned();

            let diffs = diff_fields(&create.fields, target);
            let operation = if diffs.is_empty() {
                OperationType::Skip
            } else {
                OperationType::Update
            };

            comparisons.push(RecordComparison {
                operation,
                source_id: Some(create.id),
                target_id,
                transformed: create.fields,
                diffs,
                errors: vec![],
                target_statecode,
                target_statuscode,
            });
        } else {
            // Target doesn't exist — Create
            comparisons.push(RecordComparison {
                operation: OperationType::Create,
                source_id: Some(create.id),
                target_id: None,
                transformed: create.fields,
                diffs: vec![],
                errors: vec![],
                target_statecode: None,
                target_statuscode: None,
            });
        }
    }

    log::info!(
        "[entity_lua] Processed {} independent creates: {} create, {} update, {} skip",
        comparisons.len(),
        comparisons
            .iter()
            .filter(|c| c.operation == OperationType::Create)
            .count(),
        comparisons
            .iter()
            .filter(|c| c.operation == OperationType::Update)
            .count(),
        comparisons
            .iter()
            .filter(|c| c.operation == OperationType::Skip)
            .count(),
    );

    (comparisons, matched_ids)
}

// =============================================================================
// Field parsing
// =============================================================================

/// Parse the optional `exports` table from the Lua result.
///
/// Expected shape: `{ sheet_name = { headers = { "A", "B" }, rows = { { "1", "2" }, ... } } }`
fn parse_exports_table(table: &Table) -> Result<Vec<LuaExport>, String> {
    let mut exports = Vec::new();

    for pair in table.pairs::<mlua::String, mlua::Value>() {
        let (key, value) = pair.map_err(|e| format!("Failed to iterate exports table: {e}"))?;
        let name = key
            .to_str()
            .map_err(|e| format!("Invalid UTF-8 in export name: {e}"))?
            .to_string();

        let export_table = match value {
            mlua::Value::Table(t) => t,
            other => {
                return Err(format!(
                    "Expected table for export '{name}', got {:?}",
                    other.type_name()
                ));
            }
        };

        // Parse headers
        let headers_table: Table = export_table
            .get("headers")
            .map_err(|e| format!("Export '{name}' missing 'headers': {e}"))?;
        let mut headers = Vec::new();
        for pair in headers_table.pairs::<mlua::Integer, mlua::String>() {
            let (_, val) = pair.map_err(|e| format!("Invalid header in export '{name}': {e}"))?;
            headers.push(
                val.to_str()
                    .map_err(|e| format!("Invalid UTF-8 in header: {e}"))?
                    .to_string(),
            );
        }

        // Parse rows
        let rows_table: Table = export_table
            .get("rows")
            .map_err(|e| format!("Export '{name}' missing 'rows': {e}"))?;
        let mut rows = Vec::new();
        for row_pair in rows_table.pairs::<mlua::Integer, mlua::Value>() {
            let (_, row_val) =
                row_pair.map_err(|e| format!("Invalid row in export '{name}': {e}"))?;
            let row_table = match row_val {
                mlua::Value::Table(t) => t,
                other => {
                    return Err(format!(
                        "Expected table for row in export '{name}', got {:?}",
                        other.type_name()
                    ));
                }
            };
            let mut row = Vec::new();
            for cell_pair in row_table.pairs::<mlua::Integer, mlua::Value>() {
                let (_, cell_val) =
                    cell_pair.map_err(|e| format!("Invalid cell in export '{name}': {e}"))?;
                let cell_str = match cell_val {
                    mlua::Value::String(s) => s
                        .to_str()
                        .map_err(|e| format!("Invalid UTF-8 in cell: {e}"))?
                        .to_string(),
                    mlua::Value::Integer(n) => n.to_string(),
                    mlua::Value::Number(n) => n.to_string(),
                    mlua::Value::Boolean(b) => b.to_string(),
                    mlua::Value::Nil => String::new(),
                    other => format!("{:?}", other),
                };
                row.push(cell_str);
            }
            rows.push(row);
        }

        exports.push(LuaExport {
            name,
            headers,
            rows,
        });
    }

    Ok(exports)
}

/// Parse the `fields` table from a record entry into a HashMap of Values.
pub(crate) fn parse_fields_table(
    runtime: &LuaRuntime,
    table: &Table,
) -> Result<HashMap<String, Value>, String> {
    let mut fields = HashMap::new();

    for pair in table.pairs::<mlua::String, mlua::Value>() {
        let (key, lua_val) = pair.map_err(|e| format!("Invalid entry in fields table: {e}"))?;
        let field_name = key
            .to_str()
            .map_err(|e| format!("Invalid UTF-8 in field name: {e}"))?
            .to_string();

        // Convert Lua → JSON → Value
        let json_val = runtime
            .lua_to_json(lua_val)
            .map_err(|e| format!("Failed to convert field '{field_name}' to JSON: {e}"))?;
        let value = json_field_to_value(json_val);
        fields.insert(field_name, value);
    }

    Ok(fields)
}

// =============================================================================
// Value conversion
// =============================================================================

/// Convert a `serde_json::Value` to a `dataverse_lib::model::Value` using smart heuristics.
///
/// Mirrors the logic from `dataverse-lib`'s `json_value_to_value()` (which is private):
/// - Null → Null
/// - Bool → Bool
/// - Integer → Int (i32) or Long (i64)
/// - Float → Float
/// - String → try UUID parse → Guid, else try DateTime parse → DateTime, else String
/// - Object with "id" key → EntityReference
/// - Array → Json (preserved as-is)
pub(crate) fn json_field_to_value(json: serde_json::Value) -> Value {
    match json {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(b) => Value::Bool(b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                if i >= i32::MIN as i64 && i <= i32::MAX as i64 {
                    Value::Int(i as i32)
                } else {
                    Value::Long(i)
                }
            } else if let Some(f) = n.as_f64() {
                Value::Float(f)
            } else {
                Value::Null
            }
        }
        serde_json::Value::String(s) => {
            // Try UUID
            if let Ok(uuid) = Uuid::parse_str(&s) {
                Value::Guid(uuid)
            }
            // Try DateTime (ISO 8601 / RFC 3339)
            else if let Ok(dt) = DateTime::parse_from_rfc3339(&s) {
                Value::DateTime(dt.with_timezone(&chrono::Utc))
            }
            // Plain string
            else {
                Value::String(s)
            }
        }
        serde_json::Value::Object(map) => {
            // EntityBinding detection: { bind = "entityset", id = "guid" }
            if let Some(bind_val) = map.get("bind") {
                if let Some(set_name) = bind_val.as_str() {
                    if let Some(id_val) = map.get("id") {
                        if let Some(id_str) = id_val.as_str() {
                            if let Ok(id) = Uuid::parse_str(id_str) {
                                return Value::EntityBinding(EntityBinding::new(set_name, id));
                            }
                        }
                        // bind with null id → clear the lookup
                        if id_val.is_null() {
                            return Value::EntityBinding(EntityBinding::null(set_name));
                        }
                    }
                    // bind without id → clear the lookup
                    return Value::EntityBinding(EntityBinding::null(set_name));
                }
            }
            // EntityReference detection: has "id" key (without "bind")
            if let Some(id_val) = map.get("id") {
                if let Some(id_str) = id_val.as_str() {
                    if let Ok(id) = Uuid::parse_str(id_str) {
                        // Parse entity from the "entity" key
                        let entity = map
                            .get("entity")
                            .and_then(|e| {
                                // Entity serializes as {"Logical":"name"} or similar
                                if let Some(obj) = e.as_object() {
                                    obj.get("Logical")
                                        .and_then(|v| v.as_str())
                                        .map(|s| Entity::logical(s))
                                } else if let Some(s) = e.as_str() {
                                    Some(Entity::logical(s))
                                } else {
                                    None
                                }
                            })
                            .unwrap_or_else(|| Entity::logical(""));
                        let name = map
                            .get("name")
                            .and_then(|n| n.as_str())
                            .map(|s| s.to_string());

                        return Value::EntityReference(EntityReference { id, entity, name });
                    }
                }
            }
            // Unknown object — preserve as JSON
            Value::Json(serde_json::Value::Object(map))
        }
        serde_json::Value::Array(arr) => Value::Json(serde_json::Value::Array(arr)),
    }
}

// =============================================================================
// Helpers
// =============================================================================

/// Extract the source record ID as a UUID.
fn source_record_id(record: &Record) -> Option<Uuid> {
    record.id()
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ---- json_field_to_value tests ----

    #[test]
    fn null_value() {
        assert_eq!(json_field_to_value(serde_json::Value::Null), Value::Null,);
    }

    #[test]
    fn bool_value() {
        assert_eq!(
            json_field_to_value(serde_json::json!(true)),
            Value::Bool(true),
        );
    }

    #[test]
    fn int_value() {
        assert_eq!(json_field_to_value(serde_json::json!(42)), Value::Int(42),);
    }

    #[test]
    fn long_value() {
        let big = i64::MAX;
        assert_eq!(
            json_field_to_value(serde_json::json!(big)),
            Value::Long(big),
        );
    }

    #[test]
    fn float_value() {
        assert_eq!(
            json_field_to_value(serde_json::json!(3.14)),
            Value::Float(3.14),
        );
    }

    #[test]
    fn string_value() {
        assert_eq!(
            json_field_to_value(serde_json::json!("hello")),
            Value::String("hello".to_string()),
        );
    }

    #[test]
    fn guid_string_value() {
        let uuid_str = "12345678-1234-1234-1234-123456789abc";
        let expected = Uuid::parse_str(uuid_str).unwrap();
        assert_eq!(
            json_field_to_value(serde_json::json!(uuid_str)),
            Value::Guid(expected),
        );
    }

    #[test]
    fn datetime_string_value() {
        let dt_str = "2024-01-15T10:30:00Z";
        let expected = DateTime::parse_from_rfc3339(dt_str)
            .unwrap()
            .with_timezone(&chrono::Utc);
        assert_eq!(
            json_field_to_value(serde_json::json!(dt_str)),
            Value::DateTime(expected),
        );
    }

    #[test]
    fn entity_reference_object() {
        let json = serde_json::json!({
            "id": "12345678-1234-1234-1234-123456789abc",
            "entity": {"Logical": "contact"},
            "name": "John Smith"
        });
        let value = json_field_to_value(json);
        match value {
            Value::EntityReference(er) => {
                assert_eq!(
                    er.id,
                    Uuid::parse_str("12345678-1234-1234-1234-123456789abc").unwrap()
                );
                assert_eq!(er.entity, Entity::logical("contact"));
                assert_eq!(er.name, Some("John Smith".to_string()));
            }
            other => panic!("Expected EntityReference, got {:?}", other),
        }
    }

    #[test]
    fn entity_reference_string_entity() {
        let json = serde_json::json!({
            "id": "12345678-1234-1234-1234-123456789abc",
            "entity": "contact"
        });
        let value = json_field_to_value(json);
        match value {
            Value::EntityReference(er) => {
                assert_eq!(er.entity, Entity::logical("contact"));
                assert_eq!(er.name, None);
            }
            other => panic!("Expected EntityReference, got {:?}", other),
        }
    }

    #[test]
    fn entity_binding_with_id() {
        let json = serde_json::json!({
            "bind": "systemusers",
            "id": "12345678-1234-1234-1234-123456789abc"
        });
        let value = json_field_to_value(json);
        match value {
            Value::EntityBinding(eb) => {
                assert_eq!(eb.set_name, "systemusers");
                assert_eq!(
                    eb.id,
                    Some(Uuid::parse_str("12345678-1234-1234-1234-123456789abc").unwrap())
                );
            }
            other => panic!("Expected EntityBinding, got {:?}", other),
        }
    }

    #[test]
    fn entity_binding_null_id() {
        let json = serde_json::json!({
            "bind": "contacts",
            "id": null
        });
        let value = json_field_to_value(json);
        match value {
            Value::EntityBinding(eb) => {
                assert_eq!(eb.set_name, "contacts");
                assert_eq!(eb.id, None);
            }
            other => panic!("Expected EntityBinding (null), got {:?}", other),
        }
    }

    #[test]
    fn unknown_object_preserved_as_json() {
        let json = serde_json::json!({"foo": "bar"});
        let value = json_field_to_value(json.clone());
        assert_eq!(value, Value::Json(json));
    }

    #[test]
    fn array_preserved_as_json() {
        let json = serde_json::json!([1, 2, 3]);
        let value = json_field_to_value(json.clone());
        assert_eq!(value, Value::Json(json));
    }

    // ---- execute_entity_lua tests ----

    #[test]
    fn simple_entity_lua_execution() {
        let script = r#"
local M = {}
function M.declare()
    return {
        source = "account",
        target = "account",
    }
end
function M.resolve(source, target)
    local results = {}
    for _, record in ipairs(source["account"]) do
        local id = record["accountid"]
        results[id] = {
            target = id,
            fields = { name = record["name"] },
        }
    end
    return { results = results }
end
return M
"#;

        let id1 = Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
        let id2 = Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap();
        let source_records = vec![
            Record::with_id(Entity::logical("account"), id1)
                .set("accountid", Value::Guid(id1))
                .set("name", "Acme"),
            Record::with_id(Entity::logical("account"), id2)
                .set("accountid", Value::Guid(id2))
                .set("name", "Contoso"),
        ];

        let result =
            execute_entity_lua(script, &source_records, "account", "account", &[], &[]).unwrap();

        assert_eq!(result.record_results.len(), 2);
        assert!(result.record_results[0].errors.is_empty());
        assert_eq!(
            result.record_results[0].fields.get("name"),
            Some(&Value::String("Acme".to_string())),
        );
        assert_eq!(
            result.record_results[1].fields.get("name"),
            Some(&Value::String("Contoso".to_string())),
        );
        // Both matched to themselves
        assert_eq!(result.match_index.get(&id1), Some(&id1));
        assert_eq!(result.match_index.get(&id2), Some(&id2));
    }

    #[test]
    fn entity_lua_global_error() {
        let script = r#"
local M = {}
function M.declare()
    return { source = "account", target = "account" }
end
function M.resolve(source, target)
    return { error = "something went wrong" }
end
return M
"#;

        let result = execute_entity_lua(script, &[], "account", "account", &[], &[]);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("something went wrong"));
    }

    #[test]
    fn entity_lua_per_record_error() {
        let script = r#"
local M = {}
function M.declare()
    return { source = "account", target = "account" }
end
function M.resolve(source, target)
    local results = {}
    for _, record in ipairs(source["account"]) do
        local id = record["accountid"]
        results[id] = { error = "bad record" }
    end
    return { results = results }
end
return M
"#;

        let id1 = Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
        let source_records = vec![
            Record::with_id(Entity::logical("account"), id1)
                .set("accountid", Value::Guid(id1))
                .set("name", "Acme"),
        ];

        let result =
            execute_entity_lua(script, &source_records, "account", "account", &[], &[]).unwrap();

        assert_eq!(result.record_results.len(), 1);
        assert_eq!(result.record_results[0].errors.len(), 1);
        assert_eq!(result.record_results[0].errors[0].0, "_script");
    }

    #[test]
    fn entity_lua_missing_record_in_results() {
        let script = r#"
local M = {}
function M.declare()
    return { source = "account", target = "account" }
end
function M.resolve(source, target)
    return { results = {} }
end
return M
"#;

        let id1 = Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
        let source_records = vec![
            Record::with_id(Entity::logical("account"), id1).set("accountid", Value::Guid(id1)),
        ];

        let result =
            execute_entity_lua(script, &source_records, "account", "account", &[], &[]).unwrap();

        assert_eq!(result.record_results.len(), 1);
        // Missing records are marked as skipped — the comparison engine
        // emits IgnoreSource directly without consulting matching.
        assert!(result.record_results[0].skipped);
        assert!(result.record_results[0].errors.is_empty());
        assert!(result.record_results[0].fields.is_empty());
    }

    #[test]
    fn entity_lua_nil_target_means_create() {
        let script = r#"
local M = {}
function M.declare()
    return { source = "account", target = "account" }
end
function M.resolve(source, target)
    local results = {}
    for _, record in ipairs(source["account"]) do
        local id = record["accountid"]
        results[id] = {
            fields = { name = record["name"] },
        }
    end
    return { results = results }
end
return M
"#;

        let id1 = Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
        let source_records = vec![
            Record::with_id(Entity::logical("account"), id1)
                .set("accountid", Value::Guid(id1))
                .set("name", "Acme"),
        ];

        let result =
            execute_entity_lua(script, &source_records, "account", "account", &[], &[]).unwrap();

        assert_eq!(result.record_results.len(), 1);
        assert!(result.record_results[0].errors.is_empty());
        // No match in index — means create
        assert!(result.match_index.is_empty());
    }

    #[test]
    fn entity_lua_missing_resolve_fn() {
        let script = r#"
local M = {}
function M.declare()
    return { source = "account", target = "account" }
end
return M
"#;

        let result = execute_entity_lua(script, &[], "account", "account", &[], &[]);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("M.resolve()"));
    }

    // ---- Independent creates tests ----

    #[test]
    fn entity_lua_independent_creates_detected() {
        // Script returns results keyed by IDs that are NOT in source records
        let script = r#"
local M = {}
function M.declare()
    return { source = "account", target = "account" }
end
function M.resolve(source, target)
    local results = {}
    -- Skip all source records (return nil for them)
    -- Add an independent create with a new GUID
    results["aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"] = {
        fields = { name = "Independent Record" },
    }
    return { results = results }
end
return M
"#;

        let id1 = Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
        let source_records = vec![
            Record::with_id(Entity::logical("account"), id1)
                .set("accountid", Value::Guid(id1))
                .set("name", "Acme"),
        ];

        let result =
            execute_entity_lua(script, &source_records, "account", "account", &[], &[]).unwrap();

        // Source record should be skipped (not in results)
        assert_eq!(result.record_results.len(), 1);
        assert!(result.record_results[0].skipped);

        // Independent create should be detected
        assert_eq!(result.creates.len(), 1);
        assert_eq!(
            result.creates[0].id,
            Uuid::parse_str("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee").unwrap()
        );
        assert_eq!(
            result.creates[0].fields.get("name"),
            Some(&Value::String("Independent Record".to_string()))
        );
    }

    #[test]
    fn entity_lua_mixed_source_and_independent() {
        // Script returns both source-keyed results and independent creates
        let script = r#"
local M = {}
function M.declare()
    return { source = "account", target = "account" }
end
function M.resolve(source, target)
    local results = {}
    -- Process source records normally
    for _, record in ipairs(source["account"]) do
        local id = record["accountid"]
        results[id] = {
            target = id,
            fields = { name = record["name"] },
        }
    end
    -- Also add independent creates
    results["aaaaaaaa-0000-0000-0000-000000000001"] = {
        fields = { name = "Created 1" },
    }
    results["aaaaaaaa-0000-0000-0000-000000000002"] = {
        fields = { name = "Created 2" },
    }
    return { results = results }
end
return M
"#;

        let id1 = Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
        let source_records = vec![
            Record::with_id(Entity::logical("account"), id1)
                .set("accountid", Value::Guid(id1))
                .set("name", "Acme"),
        ];

        let result =
            execute_entity_lua(script, &source_records, "account", "account", &[], &[]).unwrap();

        // Source record processed normally
        assert_eq!(result.record_results.len(), 1);
        assert!(!result.record_results[0].skipped);
        assert_eq!(result.match_index.get(&id1), Some(&id1));

        // Two independent creates
        assert_eq!(result.creates.len(), 2);
    }

    #[test]
    fn entity_lua_no_independent_creates_backward_compatible() {
        // Standard script with only source-keyed results — creates should be empty
        let script = r#"
local M = {}
function M.declare()
    return { source = "account", target = "account" }
end
function M.resolve(source, target)
    local results = {}
    for _, record in ipairs(source["account"]) do
        local id = record["accountid"]
        results[id] = {
            target = id,
            fields = { name = record["name"] },
        }
    end
    return { results = results }
end
return M
"#;

        let id1 = Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
        let source_records = vec![
            Record::with_id(Entity::logical("account"), id1)
                .set("accountid", Value::Guid(id1))
                .set("name", "Acme"),
        ];

        let result =
            execute_entity_lua(script, &source_records, "account", "account", &[], &[]).unwrap();

        assert_eq!(result.record_results.len(), 1);
        assert!(result.creates.is_empty());
    }

    // ---- process_lua_creates tests ----

    #[test]
    fn process_creates_no_target_means_create() {
        let creates = vec![LuaCreateEntry {
            id: Uuid::parse_str("aaaaaaaa-0000-0000-0000-000000000001").unwrap(),
            fields: [("name".to_string(), Value::String("New".to_string()))]
                .into_iter()
                .collect(),
        }];

        let target_records: Vec<Record> = vec![];
        let (comps, matched) = process_lua_creates(creates, &target_records, "accountid");

        assert_eq!(comps.len(), 1);
        assert_eq!(comps[0].operation, OperationType::Create);
        assert!(comps[0].target_id.is_none());
        assert_eq!(
            comps[0].source_id,
            Some(Uuid::parse_str("aaaaaaaa-0000-0000-0000-000000000001").unwrap())
        );
        assert!(matched.is_empty());
    }

    #[test]
    fn process_creates_existing_target_with_diff_means_update() {
        let create_id = Uuid::parse_str("aaaaaaaa-0000-0000-0000-000000000001").unwrap();
        let creates = vec![LuaCreateEntry {
            id: create_id,
            fields: [("name".to_string(), Value::String("Updated".to_string()))]
                .into_iter()
                .collect(),
        }];

        let target_records = vec![
            Record::with_id(Entity::logical("account"), create_id)
                .set("accountid", Value::Guid(create_id))
                .set("name", "Old"),
        ];

        let (comps, matched) = process_lua_creates(creates, &target_records, "accountid");

        assert_eq!(comps.len(), 1);
        assert_eq!(comps[0].operation, OperationType::Update);
        assert_eq!(comps[0].target_id, Some(create_id));
        assert_eq!(comps[0].diffs.len(), 1);
        assert_eq!(comps[0].diffs[0].field, "name");
        assert!(matched.contains(&create_id));
    }

    #[test]
    fn process_creates_existing_target_no_diff_means_skip() {
        let create_id = Uuid::parse_str("aaaaaaaa-0000-0000-0000-000000000001").unwrap();
        let creates = vec![LuaCreateEntry {
            id: create_id,
            fields: [("name".to_string(), Value::String("Same".to_string()))]
                .into_iter()
                .collect(),
        }];

        let target_records = vec![
            Record::with_id(Entity::logical("account"), create_id)
                .set("accountid", Value::Guid(create_id))
                .set("name", "Same"),
        ];

        let (comps, matched) = process_lua_creates(creates, &target_records, "accountid");

        assert_eq!(comps.len(), 1);
        assert_eq!(comps[0].operation, OperationType::Skip);
        assert!(comps[0].diffs.is_empty());
        assert!(matched.contains(&create_id));
    }
}
