//! Setting<T> wrapper type for reactive, auto-persisting settings.

use std::ops::Deref;
use std::sync::Arc;

use rafter::State;
use serde::{Serialize, de::DeserializeOwned};

use super::{SettingsBackend, SettingsError};

/// A reactive setting value that auto-persists to the database.
///
/// Provides:
/// - `Deref<Target = T>` for reading via `*setting`
/// - `.set(value)` for writing (updates State + persists to DB)
pub struct Setting<T> {
    /// In-memory reactive value
    value: State<T>,
    /// Database key for this setting
    key: &'static str,
    /// Backend for persistence
    backend: Arc<dyn SettingsBackend>,
}

impl<T> Setting<T>
where
    T: Serialize + DeserializeOwned + Clone + Send + Sync + 'static,
{
    /// Load a setting from the backend, or use the default if not found.
    pub async fn load(
        backend: Arc<dyn SettingsBackend>,
        key: &'static str,
        default: T,
    ) -> Result<Self, SettingsError> {
        // Try to load from DB
        let value = match backend.get_bytes(key).await? {
            Some(bytes) => {
                let (v, _): (T, _) =
                    bincode::serde::decode_from_slice(&bytes, bincode::config::standard())?;
                v
            }
            None => {
                // Not found, use default and persist it
                let bytes = bincode::serde::encode_to_vec(&default, bincode::config::standard())?;
                backend.set_bytes(key, bytes).await?;
                default
            }
        };

        Ok(Self {
            value: State::new(value),
            key,
            backend,
        })
    }

    /// Set a new value (updates in-memory state and persists to DB).
    pub async fn set(&self, new_value: T) -> Result<(), SettingsError> {
        // Update in-memory state (triggers UI reactivity)
        self.value.set(new_value.clone());

        // Serialize and persist to DB
        let bytes = bincode::serde::encode_to_vec(&new_value, bincode::config::standard())?;
        self.backend.set_bytes(self.key, bytes).await?;

        Ok(())
    }

    /// Get a clone of the current value.
    pub fn get(&self) -> T {
        self.value.get()
    }
}

impl<T> Deref for Setting<T>
where
    T: Clone,
{
    type Target = State<T>;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T> Clone for Setting<T> {
    fn clone(&self) -> Self {
        Self {
            value: self.value.clone(),
            key: self.key,
            backend: self.backend.clone(),
        }
    }
}
