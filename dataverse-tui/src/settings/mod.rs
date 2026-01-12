//! Settings system for typed key-value storage.

mod backend;
mod sqlite;

pub use backend::SettingsBackend;
pub use sqlite::SqliteBackend;

use std::sync::Arc;

use serde::{de::DeserializeOwned, Serialize};
use thiserror::Error;

/// Settings error type.
#[derive(Debug, Error)]
pub enum SettingsError {
    #[error("database error: {0}")]
    Database(#[from] async_sqlite::Error),
    #[error("serialization error: {0}")]
    Serialization(bincode::Error),
    #[error("deserialization error: {0}")]
    Deserialization(bincode::Error),
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
            Some(bytes) => Ok(Some(
                bincode::deserialize(&bytes).map_err(SettingsError::Deserialization)?,
            )),
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
    pub async fn set<T: Serialize + Sync>(&self, key: &str, value: &T) -> Result<(), SettingsError> {
        let bytes = bincode::serialize(value).map_err(SettingsError::Serialization)?;
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
