//! GUID functions: guid, is_guid

use mlua::Function;
use mlua::Lua;
use mlua::Result as LuaResult;
use mlua::Table;
use mlua::Value;
use uuid::Uuid;

pub fn register(lua: &Lua, lib: &Table) -> LuaResult<()> {
    lib.set("guid", create_guid(lua)?)?;
    lib.set("is_guid", create_is_guid(lua)?)?;
    Ok(())
}

/// lib.guid() -> string
/// Generate a new random GUID
fn create_guid(lua: &Lua) -> LuaResult<Function> {
    lua.create_function(|_, ()| Ok(Uuid::new_v4().to_string()))
}

/// lib.is_guid(value) -> bool
/// Check if value is a valid GUID string
fn create_is_guid(lua: &Lua) -> LuaResult<Function> {
    lua.create_function(|_, value: Value| match value {
        Value::String(s) => {
            let str_ref = s.to_str()?;
            Ok(Uuid::parse_str(str_ref.as_ref()).is_ok())
        }
        _ => Ok(false),
    })
}
