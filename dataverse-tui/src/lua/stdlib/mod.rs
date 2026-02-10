//! Standard library for Lua scripts (`lib.*` namespace)

mod collection;
mod date;
mod guid;
mod log;
mod string;
mod types;

use mlua::Lua;
use mlua::Result as LuaResult;

/// Register the `lib` table with all standard library functions.
pub fn register(lua: &Lua) -> LuaResult<()> {
    let lib = lua.create_table()?;

    collection::register(lua, &lib)?;
    date::register(lua, &lib)?;
    guid::register(lua, &lib)?;
    log::register(lua, &lib)?;
    string::register(lua, &lib)?;
    types::register(lua, &lib)?;

    lua.globals().set("lib", lib)?;
    Ok(())
}

/// Compare two Lua values for equality.
pub(crate) fn values_equal(a: &mlua::Value, b: &mlua::Value) -> bool {
    use mlua::Value;
    match (a, b) {
        (Value::Nil, Value::Nil) => true,
        (Value::Boolean(a), Value::Boolean(b)) => a == b,
        (Value::Integer(a), Value::Integer(b)) => a == b,
        (Value::Number(a), Value::Number(b)) => (a - b).abs() < f64::EPSILON,
        (Value::Integer(a), Value::Number(b)) | (Value::Number(b), Value::Integer(a)) => {
            (*a as f64 - b).abs() < f64::EPSILON
        }
        (Value::String(a), Value::String(b)) => a.as_bytes() == b.as_bytes(),
        _ => false,
    }
}

/// Convert a Lua value to a string key.
pub(crate) fn value_to_string(v: &mlua::Value) -> String {
    use mlua::Value;
    match v {
        Value::Nil => "nil".to_string(),
        Value::Boolean(b) => b.to_string(),
        Value::Integer(i) => i.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.to_str().map(|bs| bs.to_string()).unwrap_or_default(),
        _ => format!("{:?}", v),
    }
}
