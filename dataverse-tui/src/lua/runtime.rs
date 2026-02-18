//! Sandboxed Lua runtime

use mlua::Lua;
use mlua::StdLib;
use mlua::Table;
use mlua::Value;

use super::error::LuaError;
use super::stdlib;

/// A sandboxed Lua runtime with standard library functions.
///
/// The runtime restricts access to dangerous Lua features (io, os, debug, etc.)
/// while providing a useful standard library via the `lib.*` namespace.
pub struct LuaRuntime {
    lua: Lua,
}

impl LuaRuntime {
    /// Create a new sandboxed Lua runtime.
    ///
    /// The runtime includes limited Lua standard libraries (table, string, math, utf8)
    /// and registers the `lib.*` namespace with utility functions.
    pub fn new() -> Result<Self, LuaError> {
        // Create Lua with limited standard libraries (no io, os, debug, etc.)
        let lua = Lua::new_with(
            StdLib::TABLE | StdLib::STRING | StdLib::MATH | StdLib::UTF8,
            mlua::LuaOptions::default(),
        )
        .map_err(LuaError::creation)?;

        // Set memory limit (4GB - scripts can handle very large datasets like contacts/accounts)
        lua.set_memory_limit(4 * 1024 * 1024 * 1024)
            .map_err(LuaError::memory_limit)?;

        // Register our standard library
        stdlib::register(&lua).map_err(LuaError::stdlib)?;

        Ok(LuaRuntime { lua })
    }

    /// Get access to the underlying Lua instance.
    ///
    /// Use this to register additional functions or load scripts.
    pub fn lua(&self) -> &Lua {
        &self.lua
    }

    /// Load a Lua script and return its result.
    ///
    /// The script should return a value (typically a module table).
    pub fn load<T>(&self, script: &str) -> Result<T, LuaError>
    where
        T: for<'lua> mlua::FromLua,
    {
        self.lua.load(script).eval().map_err(LuaError::eval)
    }

    /// Execute a Lua script without returning a value.
    pub fn exec(&self, script: &str) -> Result<(), LuaError> {
        self.lua.load(script).exec().map_err(LuaError::exec)
    }

    /// Convert a JSON value to a Lua value.
    pub fn json_to_lua(&self, value: &serde_json::Value) -> Result<Value, LuaError> {
        json_to_lua_inner(&self.lua, value)
    }

    /// Convert a Lua value to a JSON value.
    pub fn lua_to_json(&self, value: Value) -> Result<serde_json::Value, LuaError> {
        lua_to_json_inner(value)
    }

    /// Create a Lua table from an iterator of key-value pairs.
    pub fn create_table(&self) -> Result<Table, LuaError> {
        self.lua.create_table().map_err(LuaError::table)
    }

    /// Register additional functions into the `lib` namespace.
    ///
    /// This allows feature-specific code to extend the standard library.
    pub fn extend_lib<F>(&self, f: F) -> Result<(), LuaError>
    where
        F: FnOnce(&Lua, &Table) -> mlua::Result<()>,
    {
        let lib: Table = self
            .lua
            .globals()
            .get("lib")
            .map_err(|_| LuaError::LibNotFound)?;
        f(&self.lua, &lib).map_err(LuaError::extend)?;
        Ok(())
    }
}

impl Default for LuaRuntime {
    fn default() -> Self {
        Self::new().expect("Failed to create default Lua runtime")
    }
}

/// Convert JSON to Lua value (internal helper).
fn json_to_lua_inner(lua: &Lua, value: &serde_json::Value) -> Result<Value, LuaError> {
    match value {
        serde_json::Value::Null => Ok(Value::Nil),
        serde_json::Value::Bool(b) => Ok(Value::Boolean(*b)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                // Luau uses i32 for integers, so we try to convert.
                // If it doesn't fit, fall back to f64.
                if let Ok(i32_val) = i32::try_from(i) {
                    Ok(Value::Integer(i32_val))
                } else {
                    Ok(Value::Number(i as f64))
                }
            } else if let Some(f) = n.as_f64() {
                Ok(Value::Number(f))
            } else {
                Ok(Value::Nil)
            }
        }
        serde_json::Value::String(s) => Ok(Value::String(
            lua.create_string(s).map_err(LuaError::conversion)?,
        )),
        serde_json::Value::Array(arr) => {
            let table = lua.create_table().map_err(LuaError::conversion)?;
            for (i, item) in arr.iter().enumerate() {
                table
                    .set(i + 1, json_to_lua_inner(lua, item)?)
                    .map_err(LuaError::conversion)?;
            }
            Ok(Value::Table(table))
        }
        serde_json::Value::Object(obj) => {
            let table = lua.create_table().map_err(LuaError::conversion)?;
            for (key, val) in obj {
                table
                    .set(key.as_str(), json_to_lua_inner(lua, val)?)
                    .map_err(LuaError::conversion)?;
            }
            Ok(Value::Table(table))
        }
    }
}

/// Convert Lua value to JSON (internal helper).
fn lua_to_json_inner(value: Value) -> Result<serde_json::Value, LuaError> {
    match value {
        Value::Nil => Ok(serde_json::Value::Null),
        Value::Boolean(b) => Ok(serde_json::Value::Bool(b)),
        Value::Integer(i) => Ok(serde_json::json!(i)),
        Value::Number(n) => Ok(serde_json::json!(n)),
        Value::String(s) => Ok(serde_json::Value::String(
            s.to_str().map_err(LuaError::conversion)?.to_string(),
        )),
        Value::Table(t) => {
            // Check if it's an array (sequential integer keys starting at 1)
            let len = t.len().map_err(LuaError::conversion)?;
            if len > 0 {
                let mut arr = Vec::new();
                let mut is_array = true;
                for i in 1..=len {
                    match t.get::<Value>(i) {
                        Ok(v) => arr.push(lua_to_json_inner(v)?),
                        Err(_) => {
                            is_array = false;
                            break;
                        }
                    }
                }
                if is_array {
                    return Ok(serde_json::Value::Array(arr));
                }
            }

            // Treat as object
            let mut obj = serde_json::Map::new();
            for pair in t.pairs::<Value, Value>() {
                let (k, v) = pair.map_err(LuaError::conversion)?;
                let key = match k {
                    Value::String(s) => s.to_str().map_err(LuaError::conversion)?.to_string(),
                    Value::Integer(i) => i.to_string(),
                    _ => continue,
                };
                obj.insert(key, lua_to_json_inner(v)?);
            }
            Ok(serde_json::Value::Object(obj))
        }
        _ => Ok(serde_json::Value::Null),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runtime_creation() {
        let runtime = LuaRuntime::new();
        assert!(runtime.is_ok());
    }

    #[test]
    fn test_sandboxing() {
        let runtime = LuaRuntime::new().unwrap();

        // io should not be available
        let result: Value = runtime.lua().load("return io").eval().unwrap();
        assert!(matches!(result, Value::Nil), "io should not be available");

        // os should not be available
        let result: Value = runtime.lua().load("return os").eval().unwrap();
        assert!(matches!(result, Value::Nil), "os should not be available");

        // debug should not be available
        let result: Value = runtime.lua().load("return debug").eval().unwrap();
        assert!(
            matches!(result, Value::Nil),
            "debug should not be available"
        );

        // package should not be available (no require)
        let result: Value = runtime.lua().load("return package").eval().unwrap();
        assert!(
            matches!(result, Value::Nil),
            "package should not be available"
        );
    }

    #[test]
    fn test_json_roundtrip() {
        let runtime = LuaRuntime::new().unwrap();

        let original = serde_json::json!({
            "name": "Test",
            "value": 42,
            "nested": {
                "array": [1, 2, 3],
                "boolean": true
            }
        });

        let lua_value = runtime.json_to_lua(&original).unwrap();
        let result = runtime.lua_to_json(lua_value).unwrap();

        assert_eq!(original, result);
    }

    #[test]
    fn test_extend_lib() {
        let runtime = LuaRuntime::new().unwrap();

        runtime
            .extend_lib(|lua, lib| {
                lib.set("custom_fn", lua.create_function(|_, ()| Ok("custom"))?)?;
                Ok(())
            })
            .unwrap();

        let result: String = runtime.load("return lib.custom_fn()").unwrap();
        assert_eq!(result, "custom");
    }
}
