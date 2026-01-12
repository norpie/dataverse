//! Settings backend trait.

use async_trait::async_trait;

use super::SettingsError;

/// Backend trait for settings storage.
///
/// Implementations handle raw byte storage/retrieval.
/// The `SettingsProvider` wraps this with typed serialization.
#[async_trait]
pub trait SettingsBackend: Send + Sync {
    /// Get raw bytes for a key.
    async fn get_bytes(&self, key: &str) -> Result<Option<Vec<u8>>, SettingsError>;

    /// Set raw bytes for a key.
    async fn set_bytes(&self, key: &str, value: Vec<u8>) -> Result<(), SettingsError>;

    /// Delete a key.
    async fn delete(&self, key: &str) -> Result<(), SettingsError>;

    /// Get all keys matching a prefix.
    async fn keys_with_prefix(&self, prefix: &str) -> Result<Vec<String>, SettingsError>;
}
