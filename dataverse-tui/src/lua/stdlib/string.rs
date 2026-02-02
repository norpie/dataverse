//! String functions: lower, upper, trim, split, contains, starts_with, ends_with

use mlua::Function;
use mlua::Lua;
use mlua::Result as LuaResult;
use mlua::Table;

pub fn register(lua: &Lua, lib: &Table) -> LuaResult<()> {
    lib.set("lower", create_lower(lua)?)?;
    lib.set("upper", create_upper(lua)?)?;
    lib.set("trim", create_trim(lua)?)?;
    lib.set("split", create_split(lua)?)?;
    lib.set("contains", create_contains(lua)?)?;
    lib.set("starts_with", create_starts_with(lua)?)?;
    lib.set("ends_with", create_ends_with(lua)?)?;
    Ok(())
}

/// lib.lower(s) -> string
fn create_lower(lua: &Lua) -> LuaResult<Function> {
    lua.create_function(|_, s: String| Ok(s.to_lowercase()))
}

/// lib.upper(s) -> string
fn create_upper(lua: &Lua) -> LuaResult<Function> {
    lua.create_function(|_, s: String| Ok(s.to_uppercase()))
}

/// lib.trim(s) -> string
fn create_trim(lua: &Lua) -> LuaResult<Function> {
    lua.create_function(|_, s: String| Ok(s.trim().to_string()))
}

/// lib.split(s, delim) -> table
fn create_split(lua: &Lua) -> LuaResult<Function> {
    lua.create_function(|lua, (s, delim): (String, String)| {
        let result = lua.create_table()?;
        for (i, part) in s.split(&delim).enumerate() {
            result.set(i + 1, part)?;
        }
        Ok(result)
    })
}

/// lib.contains(s, sub) -> bool
fn create_contains(lua: &Lua) -> LuaResult<Function> {
    lua.create_function(|_, (s, sub): (String, String)| Ok(s.contains(&sub)))
}

/// lib.starts_with(s, prefix) -> bool
fn create_starts_with(lua: &Lua) -> LuaResult<Function> {
    lua.create_function(|_, (s, prefix): (String, String)| Ok(s.starts_with(&prefix)))
}

/// lib.ends_with(s, suffix) -> bool
fn create_ends_with(lua: &Lua) -> LuaResult<Function> {
    lua.create_function(|_, (s, suffix): (String, String)| Ok(s.ends_with(&suffix)))
}
