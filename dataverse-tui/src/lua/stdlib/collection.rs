//! Collection functions: find, filter, map, group_by

use mlua::Function;
use mlua::Lua;
use mlua::Result as LuaResult;
use mlua::Table;
use mlua::Value;

use super::value_to_string;
use super::values_equal;

pub fn register(lua: &Lua, lib: &Table) -> LuaResult<()> {
    lib.set("find", create_find(lua)?)?;
    lib.set("filter", create_filter(lua)?)?;
    lib.set("map", create_map(lua)?)?;
    lib.set("group_by", create_group_by(lua)?)?;
    Ok(())
}

/// lib.find(records, field, value) -> record|nil
/// Find first record where record[field] == value
fn create_find(lua: &Lua) -> LuaResult<Function> {
    lua.create_function(|_, (records, field, value): (Table, String, Value)| {
        for pair in records.pairs::<Value, Table>() {
            if let Ok((_, record)) = pair
                && let Ok(field_value) = record.get::<Value>(field.as_str())
                    && values_equal(&field_value, &value) {
                        return Ok(Value::Table(record));
                    }
        }
        Ok(Value::Nil)
    })
}

/// lib.filter(records, fn) -> records
/// Filter records by predicate function
fn create_filter(lua: &Lua) -> LuaResult<Function> {
    lua.create_function(|lua, (records, predicate): (Table, Function)| {
        let result = lua.create_table()?;
        let mut idx = 1;
        for (_, record) in records.pairs::<Value, Value>().flatten() {
            let keep: bool = predicate.call(record.clone())?;
            if keep {
                result.set(idx, record)?;
                idx += 1;
            }
        }
        Ok(result)
    })
}

/// lib.map(records, fn) -> records
/// Transform each record using function
fn create_map(lua: &Lua) -> LuaResult<Function> {
    lua.create_function(|lua, (records, transform): (Table, Function)| {
        let result = lua.create_table()?;
        let mut idx = 1;
        for (_, record) in records.pairs::<Value, Value>().flatten() {
            let transformed: Value = transform.call(record)?;
            result.set(idx, transformed)?;
            idx += 1;
        }
        Ok(result)
    })
}

/// lib.group_by(records, field) -> table
/// Group records by field value
fn create_group_by(lua: &Lua) -> LuaResult<Function> {
    lua.create_function(|lua, (records, field): (Table, String)| {
        let result = lua.create_table()?;

        for pair in records.pairs::<Value, Table>() {
            if let Ok((_, record)) = pair
                && let Ok(key) = record.get::<Value>(field.as_str()) {
                    let key_str = value_to_string(&key);

                    // Get or create the group
                    let group: Table = match result.get::<Table>(key_str.as_str()) {
                        Ok(g) => g,
                        Err(_) => {
                            let g = lua.create_table()?;
                            result.set(key_str.as_str(), g.clone())?;
                            g
                        }
                    };

                    // Add record to group
                    let len = group.len()? + 1;
                    group.set(len, record)?;
                }
        }
        Ok(result)
    })
}
