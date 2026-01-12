//! SQLite settings backend with in-memory cache.

use std::path::Path;

use async_sqlite::Client;
use async_trait::async_trait;
use dashmap::DashMap;

use super::{SettingsBackend, SettingsError};

/// SQLite-backed settings storage with DashMap cache.
pub struct SqliteBackend {
    client: Client,
    cache: DashMap<String, Vec<u8>>,
}

impl SqliteBackend {
    /// Create a new SQLite backend at the given path.
    pub async fn new(path: impl AsRef<Path>) -> Result<Self, SettingsError> {
        let client = async_sqlite::ClientBuilder::new()
            .path(path)
            .open()
            .await?;

        client
            .conn(|conn| {
                conn.execute(
                    "CREATE TABLE IF NOT EXISTS settings (
                        key TEXT PRIMARY KEY,
                        value BLOB NOT NULL
                    )",
                    [],
                )
            })
            .await?;

        Ok(Self {
            client,
            cache: DashMap::new(),
        })
    }
}

#[async_trait]
impl SettingsBackend for SqliteBackend {
    async fn get_bytes(&self, key: &str) -> Result<Option<Vec<u8>>, SettingsError> {
        // Check cache first
        if let Some(value) = self.cache.get(key) {
            return Ok(Some(value.clone()));
        }

        // Cache miss - query DB
        let key_owned = key.to_string();
        let result = self
            .client
            .conn(move |conn| {
                let mut stmt = conn.prepare("SELECT value FROM settings WHERE key = ?")?;
                let mut rows = stmt.query([&key_owned])?;
                match rows.next()? {
                    Some(row) => Ok(Some(row.get::<_, Vec<u8>>(0)?)),
                    None => Ok(None),
                }
            })
            .await?;

        // Populate cache
        if let Some(ref value) = result {
            self.cache.insert(key.to_string(), value.clone());
        }

        Ok(result)
    }

    async fn set_bytes(&self, key: &str, value: Vec<u8>) -> Result<(), SettingsError> {
        let key_owned = key.to_string();
        let value_clone = value.clone();

        self.client
            .conn(move |conn| {
                conn.execute(
                    "INSERT INTO settings (key, value) VALUES (?, ?)
                     ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                    rusqlite::params![&key_owned, &value_clone],
                )
            })
            .await?;

        // Update cache
        self.cache.insert(key.to_string(), value);

        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<(), SettingsError> {
        let key_owned = key.to_string();

        self.client
            .conn(move |conn| conn.execute("DELETE FROM settings WHERE key = ?", [&key_owned]))
            .await?;

        // Remove from cache
        self.cache.remove(key);

        Ok(())
    }

    async fn keys_with_prefix(&self, prefix: &str) -> Result<Vec<String>, SettingsError> {
        let pattern = format!("{}%", prefix);
        self.client
            .conn(move |conn| {
                let mut stmt = conn.prepare("SELECT key FROM settings WHERE key LIKE ?")?;
                let rows = stmt.query_map([&pattern], |row| row.get(0))?;
                rows.collect::<Result<Vec<_>, _>>()
            })
            .await
            .map_err(SettingsError::from)
    }
}
