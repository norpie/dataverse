//! Logging functions: log.info, log.warn, log.error, log.debug

use mlua::Function;
use mlua::Lua;
use mlua::Result as LuaResult;
use mlua::Table;

pub fn register(lua: &Lua, lib: &Table) -> LuaResult<()> {
    let log_table = lua.create_table()?;

    log_table.set("info", create_log_info(lua)?)?;
    log_table.set("warn", create_log_warn(lua)?)?;
    log_table.set("error", create_log_error(lua)?)?;
    log_table.set("debug", create_log_debug(lua)?)?;

    lib.set("log", log_table)?;
    Ok(())
}

/// lib.log.info(msg)
fn create_log_info(lua: &Lua) -> LuaResult<Function> {
    lua.create_function(|_, msg: String| {
        log::info!("[Lua] {}", msg);
        Ok(())
    })
}

/// lib.log.warn(msg)
fn create_log_warn(lua: &Lua) -> LuaResult<Function> {
    lua.create_function(|_, msg: String| {
        log::warn!("[Lua] {}", msg);
        Ok(())
    })
}

/// lib.log.error(msg)
fn create_log_error(lua: &Lua) -> LuaResult<Function> {
    lua.create_function(|_, msg: String| {
        log::error!("[Lua] {}", msg);
        Ok(())
    })
}

/// lib.log.debug(msg)
fn create_log_debug(lua: &Lua) -> LuaResult<Function> {
    lua.create_function(|_, msg: String| {
        log::debug!("[Lua] {}", msg);
        Ok(())
    })
}
