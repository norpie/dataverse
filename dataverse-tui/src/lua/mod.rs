//! Lua runtime for scripting support
//!
//! Provides a sandboxed Lua environment with a standard library (`lib.*`).
//! Used by the migration app and potentially other Lua-based features.

mod error;
mod runtime;
mod stdlib;

pub use error::LuaError;
pub use runtime::LuaRuntime;
