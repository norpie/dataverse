//! Phase-level Lua execution — a single script produces operations directly.
//!
//! Unlike entity-level Lua (which outputs desired field values for the comparison engine),
//! phase-level Lua outputs explicit operations: create, update, delete, activate, deactivate,
//! associate, disassociate. These bypass the comparison/preview pipeline entirely.

use std::collections::HashMap;

use dataverse_lib::model::Record;
use dataverse_lib::model::Value;
use mlua::Table;
use uuid::Uuid;

use super::entity_lua::parse_fields_table;
use super::matching::build_entity_table;
use crate::apps::migration::execution::phase_lua::PhaseLuaOperation;
use crate::lua::runtime::LuaRuntime;

// =============================================================================
// Public API
// =============================================================================

/// Execute a phase-level Lua script and return the parsed operations.
///
/// 1. Creates a sandboxed Lua runtime and loads the script.
/// 2. Builds entity-keyed tables from all source/target records.
/// 3. Calls `M.resolve(source, target)` to get an operations list.
/// 4. Parses each operation into a `PhaseLuaOperation`.
///
/// Returns `Err` if the script fails or returns `{ error = "..." }`.
pub fn execute_phase_lua(
    script: &str,
    source_data: &HashMap<String, Vec<Record>>,
    target_data: &HashMap<String, Vec<Record>>,
) -> Result<Vec<PhaseLuaOperation>, String> {
    let runtime = LuaRuntime::new().map_err(|e| format!("Failed to create Lua runtime: {e}"))?;

    // Load the script module
    let module: Table = runtime
        .load(script)
        .map_err(|e| format!("Failed to load Lua script: {e}"))?;

    // Build source/target entity tables
    let source_refs: HashMap<String, &[Record]> = source_data
        .iter()
        .map(|(k, v)| (k.clone(), v.as_slice()))
        .collect();
    let target_refs: HashMap<String, &[Record]> = target_data
        .iter()
        .map(|(k, v)| (k.clone(), v.as_slice()))
        .collect();

    let source_table = build_entity_table(&runtime, &source_refs)?;
    let target_table = build_entity_table(&runtime, &target_refs)?;

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

    // Parse operations list
    let operations_table: Table = result
        .get("operations")
        .map_err(|e| format!("Result missing 'operations' field: {e}"))?;

    let mut operations = Vec::new();

    for pair in operations_table.pairs::<i64, Table>() {
        let (idx, op_table) = pair.map_err(|e| format!("Invalid entry in operations list: {e}"))?;

        let op = parse_operation(&runtime, &op_table)
            .map_err(|e| format!("Invalid operation at index {idx}: {e}"))?;

        operations.push(op);
    }

    log::info!(
        "[phase_lua] Executed: {} operations returned",
        operations.len(),
    );

    Ok(operations)
}

// =============================================================================
// Operation parsing
// =============================================================================

/// Parse a single operation table into a `PhaseLuaOperation`.
fn parse_operation(runtime: &LuaRuntime, table: &Table) -> Result<PhaseLuaOperation, String> {
    let op_str: String = table
        .get::<mlua::String>("op")
        .map_err(|e| format!("Missing 'op' field: {e}"))?
        .to_str()
        .map_err(|e| format!("Invalid UTF-8 in 'op' field: {e}"))?
        .to_string();

    match op_str.as_str() {
        "create" => parse_create_op(runtime, table),
        "update" => parse_update_op(runtime, table),
        "activate" => parse_activate_op(table),
        "deactivate" => parse_deactivate_op(table),
        "delete" => parse_delete_op(table),
        "associate" => parse_associate_op(table),
        "disassociate" => parse_disassociate_op(table),
        other => Err(format!("Unknown operation type: '{other}'")),
    }
}

/// Parse `{ op = "create", entity = "...", id = "...", fields = { ... } }`
fn parse_create_op(runtime: &LuaRuntime, table: &Table) -> Result<PhaseLuaOperation, String> {
    let entity = parse_string_field(table, "entity")?;
    let id = parse_uuid_field(table, "id")?;
    let fields = parse_op_fields(runtime, table)?;

    Ok(PhaseLuaOperation::Create { entity, id, fields })
}

/// Parse `{ op = "update", entity = "...", id = "...", fields = { ... } }`
fn parse_update_op(runtime: &LuaRuntime, table: &Table) -> Result<PhaseLuaOperation, String> {
    let entity = parse_string_field(table, "entity")?;
    let id = parse_uuid_field(table, "id")?;
    let fields = parse_op_fields(runtime, table)?;

    Ok(PhaseLuaOperation::Update { entity, id, fields })
}

/// Parse `{ op = "activate", entity = "...", id = "..." }`
fn parse_activate_op(table: &Table) -> Result<PhaseLuaOperation, String> {
    let entity = parse_string_field(table, "entity")?;
    let id = parse_uuid_field(table, "id")?;

    Ok(PhaseLuaOperation::Activate { entity, id })
}

/// Parse `{ op = "deactivate", entity = "...", id = "..." }`
fn parse_deactivate_op(table: &Table) -> Result<PhaseLuaOperation, String> {
    let entity = parse_string_field(table, "entity")?;
    let id = parse_uuid_field(table, "id")?;

    Ok(PhaseLuaOperation::Deactivate { entity, id })
}

/// Parse `{ op = "delete", entity = "...", id = "..." }`
fn parse_delete_op(table: &Table) -> Result<PhaseLuaOperation, String> {
    let entity = parse_string_field(table, "entity")?;
    let id = parse_uuid_field(table, "id")?;

    Ok(PhaseLuaOperation::Delete { entity, id })
}

/// Parse `{ op = "associate", entity1 = "...", id1 = "...", entity2 = "...", id2 = "...", relationship = "..." }`
fn parse_associate_op(table: &Table) -> Result<PhaseLuaOperation, String> {
    let entity1 = parse_string_field(table, "entity1")?;
    let id1 = parse_uuid_field(table, "id1")?;
    let entity2 = parse_string_field(table, "entity2")?;
    let id2 = parse_uuid_field(table, "id2")?;
    let relationship = parse_string_field(table, "relationship")?;

    Ok(PhaseLuaOperation::Associate {
        entity1,
        id1,
        entity2,
        id2,
        relationship,
    })
}

/// Parse `{ op = "disassociate", entity1 = "...", id1 = "...", entity2 = "...", id2 = "...", relationship = "..." }`
fn parse_disassociate_op(table: &Table) -> Result<PhaseLuaOperation, String> {
    let entity1 = parse_string_field(table, "entity1")?;
    let id1 = parse_uuid_field(table, "id1")?;
    let entity2 = parse_string_field(table, "entity2")?;
    let id2 = parse_uuid_field(table, "id2")?;
    let relationship = parse_string_field(table, "relationship")?;

    Ok(PhaseLuaOperation::Disassociate {
        entity1,
        id1,
        entity2,
        id2,
        relationship,
    })
}

// =============================================================================
// Field helpers
// =============================================================================

/// Parse a fields table from an operation, converting Lua values to `Value`.
fn parse_op_fields(runtime: &LuaRuntime, table: &Table) -> Result<HashMap<String, Value>, String> {
    let fields_table: Table = table
        .get("fields")
        .map_err(|e| format!("Missing 'fields' table: {e}"))?;

    parse_fields_table(runtime, &fields_table)
}

/// Extract a string field from a Lua table.
fn parse_string_field(table: &Table, field: &str) -> Result<String, String> {
    let value: mlua::String = table
        .get(field)
        .map_err(|e| format!("Missing '{field}' field: {e}"))?;

    value
        .to_str()
        .map_err(|e| format!("Invalid UTF-8 in '{field}' field: {e}"))
        .map(|s| s.to_string())
}

/// Extract a UUID field from a Lua table (string value parsed as UUID).
fn parse_uuid_field(table: &Table, field: &str) -> Result<Uuid, String> {
    let value = parse_string_field(table, field)?;

    Uuid::parse_str(&value).map_err(|e| format!("Invalid UUID in '{field}' field '{value}': {e}"))
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_create_operations() {
        let script = r#"
local M = {}
function M.resolve(source, target)
    local ops = {}
    for _, record in ipairs(source["account"]) do
        local id = record["accountid"]
        table.insert(ops, {
            op = "create",
            entity = "account",
            id = id,
            fields = { name = record["name"] },
        })
    end
    return { operations = ops }
end
return M
"#;

        let id1 = Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
        let id2 = Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap();

        let mut source_data = HashMap::new();
        source_data.insert(
            "account".to_string(),
            vec![
                Record::with_id(dataverse_lib::model::Entity::logical("account"), id1)
                    .set("accountid", Value::Guid(id1))
                    .set("name", "Acme"),
                Record::with_id(dataverse_lib::model::Entity::logical("account"), id2)
                    .set("accountid", Value::Guid(id2))
                    .set("name", "Contoso"),
            ],
        );
        let target_data = HashMap::new();

        let ops = execute_phase_lua(script, &source_data, &target_data).unwrap();
        assert_eq!(ops.len(), 2);

        match &ops[0] {
            PhaseLuaOperation::Create { entity, id, fields } => {
                assert_eq!(entity, "account");
                assert_eq!(*id, id1);
                assert_eq!(fields.get("name"), Some(&Value::String("Acme".to_string())),);
            }
            other => panic!("Expected Create, got {:?}", other),
        }

        match &ops[1] {
            PhaseLuaOperation::Create { entity, id, fields } => {
                assert_eq!(entity, "account");
                assert_eq!(*id, id2);
                assert_eq!(
                    fields.get("name"),
                    Some(&Value::String("Contoso".to_string())),
                );
            }
            other => panic!("Expected Create, got {:?}", other),
        }
    }

    #[test]
    fn mixed_operations() {
        let script = r#"
local M = {}
function M.resolve(source, target)
    return {
        operations = {
            { op = "create", entity = "account", id = "11111111-1111-1111-1111-111111111111", fields = { name = "New" } },
            { op = "update", entity = "account", id = "22222222-2222-2222-2222-222222222222", fields = { name = "Updated" } },
            { op = "activate", entity = "account", id = "33333333-3333-3333-3333-333333333333" },
            { op = "deactivate", entity = "account", id = "44444444-4444-4444-4444-444444444444" },
            { op = "delete", entity = "account", id = "55555555-5555-5555-5555-555555555555" },
            { op = "associate", entity1 = "account", id1 = "11111111-1111-1111-1111-111111111111", entity2 = "contact", id2 = "22222222-2222-2222-2222-222222222222", relationship = "account_contacts" },
            { op = "disassociate", entity1 = "account", id1 = "33333333-3333-3333-3333-333333333333", entity2 = "contact", id2 = "44444444-4444-4444-4444-444444444444", relationship = "account_contacts" },
        },
    }
end
return M
"#;

        let source_data = HashMap::new();
        let target_data = HashMap::new();
        let ops = execute_phase_lua(script, &source_data, &target_data).unwrap();

        assert_eq!(ops.len(), 7);
        assert!(matches!(&ops[0], PhaseLuaOperation::Create { .. }));
        assert!(matches!(&ops[1], PhaseLuaOperation::Update { .. }));
        assert!(matches!(&ops[2], PhaseLuaOperation::Activate { .. }));
        assert!(matches!(&ops[3], PhaseLuaOperation::Deactivate { .. }));
        assert!(matches!(&ops[4], PhaseLuaOperation::Delete { .. }));
        assert!(matches!(&ops[5], PhaseLuaOperation::Associate { .. }));
        assert!(matches!(&ops[6], PhaseLuaOperation::Disassociate { .. }));
    }

    #[test]
    fn global_error() {
        let script = r#"
local M = {}
function M.resolve(source, target)
    return { error = "something went wrong" }
end
return M
"#;
        let result = execute_phase_lua(script, &HashMap::new(), &HashMap::new());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("something went wrong"));
    }

    #[test]
    fn missing_resolve() {
        let script = r#"
local M = {}
return M
"#;
        let result = execute_phase_lua(script, &HashMap::new(), &HashMap::new());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("M.resolve()"));
    }

    #[test]
    fn unknown_operation_type() {
        let script = r#"
local M = {}
function M.resolve(source, target)
    return {
        operations = {
            { op = "explode", entity = "account", id = "11111111-1111-1111-1111-111111111111" },
        },
    }
end
return M
"#;
        let result = execute_phase_lua(script, &HashMap::new(), &HashMap::new());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown operation type"));
    }

    #[test]
    fn invalid_uuid() {
        let script = r#"
local M = {}
function M.resolve(source, target)
    return {
        operations = {
            { op = "delete", entity = "account", id = "not-a-uuid" },
        },
    }
end
return M
"#;
        let result = execute_phase_lua(script, &HashMap::new(), &HashMap::new());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid UUID"));
    }

    #[test]
    fn entity_binding_in_fields() {
        let script = r#"
local M = {}
function M.resolve(source, target)
    return {
        operations = {
            {
                op = "create",
                entity = "contact",
                id = "11111111-1111-1111-1111-111111111111",
                fields = {
                    fullname = "John",
                    parentcustomerid = { bind = "accounts", id = "22222222-2222-2222-2222-222222222222" },
                },
            },
        },
    }
end
return M
"#;

        let ops = execute_phase_lua(script, &HashMap::new(), &HashMap::new()).unwrap();
        assert_eq!(ops.len(), 1);

        if let PhaseLuaOperation::Create { fields, .. } = &ops[0] {
            assert_eq!(
                fields.get("fullname"),
                Some(&Value::String("John".to_string())),
            );
            // EntityBinding via { bind, id } table
            match fields.get("parentcustomerid") {
                Some(Value::EntityBinding(eb)) => {
                    assert_eq!(eb.set_name, "accounts");
                    assert_eq!(
                        eb.id,
                        Some(Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap()),
                    );
                }
                other => panic!("Expected EntityBinding, got {:?}", other),
            }
        } else {
            panic!("Expected Create");
        }
    }
}
