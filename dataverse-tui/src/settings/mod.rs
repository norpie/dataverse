//! Settings system for typed key-value storage.

mod backend;
pub mod migrations;
mod sqlite;

pub use backend::SettingsBackend;
pub use sqlite::SqliteBackend;

use std::sync::Arc;

use serde::{Serialize, de::DeserializeOwned};
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

/// Typed settings provider.
///
/// Wraps a `SettingsBackend` with typed serialization via bincode.
#[derive(Clone)]
pub struct SettingsProvider {
    backend: Arc<dyn SettingsBackend>,
}

impl SettingsProvider {
    /// Create a new settings provider with the given backend.
    pub fn new(backend: impl SettingsBackend + 'static) -> Self {
        Self {
            backend: Arc::new(backend),
        }
    }

    /// Get a typed value for a key.
    pub async fn get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>, SettingsError> {
        match self.backend.get_bytes(key).await? {
            Some(bytes) => {
                let (value, _): (T, _) =
                    bincode::serde::decode_from_slice(&bytes, bincode::config::standard())?;
                Ok(Some(value))
            }
            None => Ok(None),
        }
    }

    /// Get a typed value for a key, returning a default if not found.
    pub async fn get_or<T: DeserializeOwned>(
        &self,
        key: &str,
        default: T,
    ) -> Result<T, SettingsError> {
        Ok(self.get(key).await?.unwrap_or(default))
    }

    /// Set a typed value for a key.
    pub async fn set<T: Serialize + Sync>(
        &self,
        key: &str,
        value: &T,
    ) -> Result<(), SettingsError> {
        let bytes = bincode::serde::encode_to_vec(value, bincode::config::standard())?;
        self.backend.set_bytes(key, bytes).await
    }

    /// Delete a key.
    pub async fn delete(&self, key: &str) -> Result<(), SettingsError> {
        self.backend.delete(key).await
    }

    /// Get all keys matching a prefix.
    pub async fn keys_with_prefix(&self, prefix: &str) -> Result<Vec<String>, SettingsError> {
        self.backend.keys_with_prefix(prefix).await
    }
}
