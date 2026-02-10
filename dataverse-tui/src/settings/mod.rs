//! Settings system for typed key-value storage.

mod backend;
pub mod migrations;
mod setting;
mod sqlite;
mod types;

pub use backend::SettingsBackend;
pub use setting::Setting;
pub use sqlite::SqliteBackend;
pub use types::Settings;

// Re-export the macro

use thiserror::Error;

/// Settings error type.
#[derive(Debug, Error)]
pub enum SettingsError {
    #[error("database error: {0}")]
    Database(#[from] async_sqlite::Error),
    #[error("migration error: {0}")]
    Migration(#[from] crate::migrations::MigrationError),
    #[error("serialization error: {0}")]
    Serialization(#[from] bincode::error::EncodeError),
    #[error("deserialization error: {0}")]
    Deserialization(#[from] bincode::error::DecodeError),
}
