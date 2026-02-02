//! Type check functions: is_nil, is_string, is_number, is_table, is_boolean

use mlua::Function;
use mlua::Lua;
use mlua::Result as LuaResult;
use mlua::Table;
use mlua::Value;

pub fn register(lua: &Lua, lib: &Table) -> LuaResult<()> {
    lib.set("is_nil", create_is_nil(lua)?)?;
    lib.set("is_string", create_is_string(lua)?)?;
    lib.set("is_number", create_is_number(lua)?)?;
    lib.set("is_table", create_is_table(lua)?)?;
    lib.set("is_boolean", create_is_boolean(lua)?)?;
    Ok(())
}

/// lib.is_nil(v) -> bool
fn create_is_nil(lua: &Lua) -> LuaResult<Function> {
    lua.create_function(|_, v: Value| Ok(matches!(v, Value::Nil)))
}

/// lib.is_string(v) -> bool
fn create_is_string(lua: &Lua) -> LuaResult<Function> {
    lua.create_function(|_, v: Value| Ok(matches!(v, Value::String(_))))
}

/// lib.is_number(v) -> bool
fn create_is_number(lua: &Lua) -> LuaResult<Function> {
    lua.create_function(|_, v: Value| Ok(matches!(v, Value::Number(_) | Value::Integer(_))))
}

/// lib.is_table(v) -> bool
fn create_is_table(lua: &Lua) -> LuaResult<Function> {
    lua.create_function(|_, v: Value| Ok(matches!(v, Value::Table(_))))
}

/// lib.is_boolean(v) -> bool
fn create_is_boolean(lua: &Lua) -> LuaResult<Function> {
    lua.create_function(|_, v: Value| Ok(matches!(v, Value::Boolean(_))))
}
