//! Lua runtime errors.

use thiserror::Error;

/// Errors from Lua runtime operations.
#[derive(Debug, Error)]
pub enum LuaError {
    #[error("Failed to create Lua runtime: {0}")]
    Creation(String),

    #[error("Failed to set memory limit: {0}")]
    MemoryLimit(String),

    #[error("Failed to register stdlib: {0}")]
    StdlibRegistration(String),

    #[error("Failed to evaluate script: {0}")]
    Eval(String),

    #[error("Failed to execute script: {0}")]
    Exec(String),

    #[error("Failed to create table: {0}")]
    TableCreation(String),

    #[error("Failed to extend lib: {0}")]
    ExtendLib(String),

    #[error("Failed to convert value: {0}")]
    Conversion(String),

    #[error("lib table not found")]
    LibNotFound,
}

impl LuaError {
    /// Create an error from an mlua::Error for runtime creation.
    pub fn creation(e: mlua::Error) -> Self {
        Self::Creation(e.to_string())
    }

    /// Create an error from an mlua::Error for memory limit.
    pub fn memory_limit(e: mlua::Error) -> Self {
        Self::MemoryLimit(e.to_string())
    }

    /// Create an error from an mlua::Error for stdlib registration.
    pub fn stdlib(e: mlua::Error) -> Self {
        Self::StdlibRegistration(e.to_string())
    }

    /// Create an error from an mlua::Error for eval.
    pub fn eval(e: mlua::Error) -> Self {
        Self::Eval(e.to_string())
    }

    /// Create an error from an mlua::Error for exec.
    pub fn exec(e: mlua::Error) -> Self {
        Self::Exec(e.to_string())
    }

    /// Create an error from an mlua::Error for table creation.
    pub fn table(e: mlua::Error) -> Self {
        Self::TableCreation(e.to_string())
    }

    /// Create an error from an mlua::Error for extending lib.
    pub fn extend(e: mlua::Error) -> Self {
        Self::ExtendLib(e.to_string())
    }

    /// Create an error from an mlua::Error for conversion.
    pub fn conversion(e: mlua::Error) -> Self {
        Self::Conversion(e.to_string())
    }
}
