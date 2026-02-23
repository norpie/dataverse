//! GUID functions: guid, deterministic_guid, is_guid

use mlua::Function;
use mlua::Lua;
use mlua::Result as LuaResult;
use mlua::Table;
use mlua::Value;
use uuid::Uuid;

/// Namespace UUID for deterministic GUID generation (randomly generated, fixed).
const MIGRATION_NAMESPACE: Uuid = Uuid::from_bytes([
    0x8a, 0x3b, 0x4c, 0x5d, 0x6e, 0x7f, 0x40, 0x91, 0xa2, 0xb3, 0xc4, 0xd5, 0xe6, 0xf7, 0x08, 0x19,
]);

pub fn register(lua: &Lua, lib: &Table) -> LuaResult<()> {
    lib.set("guid", create_guid(lua)?)?;
    lib.set("deterministic_guid", create_deterministic_guid(lua)?)?;
    lib.set("is_guid", create_is_guid(lua)?)?;
    Ok(())
}

/// lib.guid() -> string
/// Generate a new random GUID
fn create_guid(lua: &Lua) -> LuaResult<Function> {
    lua.create_function(|_, ()| Ok(Uuid::new_v4().to_string()))
}

/// lib.deterministic_guid(input) -> string
/// Generate a deterministic GUID (UUID v5) from the input string.
/// The same input always produces the same GUID.
fn create_deterministic_guid(lua: &Lua) -> LuaResult<Function> {
    lua.create_function(|_, input: String| {
        Ok(Uuid::new_v5(&MIGRATION_NAMESPACE, input.as_bytes()).to_string())
    })
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
