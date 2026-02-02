//! Date functions: now, parse_date, format_date

use chrono::NaiveDate;
use chrono::NaiveDateTime;
use chrono::Utc;
use mlua::Function;
use mlua::Lua;
use mlua::Result as LuaResult;
use mlua::Table;
use mlua::Value;

pub fn register(lua: &Lua, lib: &Table) -> LuaResult<()> {
    lib.set("now", create_now(lua)?)?;
    lib.set("parse_date", create_parse_date(lua)?)?;
    lib.set("format_date", create_format_date(lua)?)?;
    Ok(())
}

/// lib.now() -> string
/// Returns current UTC time in ISO 8601 format
fn create_now(lua: &Lua) -> LuaResult<Function> {
    lua.create_function(|_, ()| Ok(Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()))
}

/// lib.parse_date(s) -> string|nil
/// Parse various date formats to ISO 8601
fn create_parse_date(lua: &Lua) -> LuaResult<Function> {
    lua.create_function(|lua, s: String| {
        // Try common formats
        let formats = [
            "%Y-%m-%dT%H:%M:%S%.fZ",
            "%Y-%m-%dT%H:%M:%SZ",
            "%Y-%m-%dT%H:%M:%S",
            "%Y-%m-%d %H:%M:%S",
            "%Y-%m-%d",
            "%d/%m/%Y %H:%M:%S",
            "%d/%m/%Y",
            "%m/%d/%Y %H:%M:%S",
            "%m/%d/%Y",
        ];

        for fmt in formats {
            if let Ok(dt) = NaiveDateTime::parse_from_str(&s, fmt) {
                let result = dt.format("%Y-%m-%dT%H:%M:%SZ").to_string();
                return Ok(Value::String(lua.create_string(&result)?));
            }
            // Try date only
            if let Ok(d) = NaiveDate::parse_from_str(&s, fmt) {
                let result = d.format("%Y-%m-%dT00:00:00Z").to_string();
                return Ok(Value::String(lua.create_string(&result)?));
            }
        }

        Ok(Value::Nil)
    })
}

/// lib.format_date(dt, fmt) -> string|nil
/// Format ISO date string with given format
fn create_format_date(lua: &Lua) -> LuaResult<Function> {
    lua.create_function(|_, (dt, fmt): (String, String)| {
        if let Ok(parsed) = NaiveDateTime::parse_from_str(&dt, "%Y-%m-%dT%H:%M:%SZ") {
            Ok(Some(parsed.format(&fmt).to_string()))
        } else if let Ok(parsed) = NaiveDateTime::parse_from_str(&dt, "%Y-%m-%dT%H:%M:%S%.fZ") {
            Ok(Some(parsed.format(&fmt).to_string()))
        } else {
            Ok(None)
        }
    })
}
