//! SQLite-backed persistent cache implementation.

use std::path::Path;

use async_sqlite::rusqlite;
use async_sqlite::Client;
use async_sqlite::ClientBuilder;
use async_sqlite::JournalMode;
use async_trait::async_trait;
use chrono::TimeZone;
use chrono::Utc;

use super::CacheProvider;
use super::CachedValue;

/// A persistent cache backed by SQLite.
///
/// Data is stored in a SQLite database file and persists across process restarts.
/// Uses WAL journal mode for better concurrent read performance.
///
/// # Example
///
/// ```ignore
/// use dataverse_lib::cache::SqliteCache;
///
/// // File-based cache
/// let cache = SqliteCache::open("cache.db").await?;
///
/// // In-memory cache (for testing)
/// let cache = SqliteCache::open_in_memory().await?;
/// ```
pub struct SqliteCache {
    client: Client,
}

impl SqliteCache {
    /// Opens a SQLite cache at the specified path.
    ///
    /// Creates the database file and cache table if they don't exist.
    pub async fn open(path: impl AsRef<Path>) -> Result<Self, async_sqlite::Error> {
        let client = ClientBuilder::new()
            .path(path)
            .journal_mode(JournalMode::Wal)
            .open()
            .await?;

        Self::init_schema(&client).await?;

        Ok(Self { client })
    }

    /// Opens an in-memory SQLite cache.
    ///
    /// Useful for testing. Data is lost when the cache is dropped.
    pub async fn open_in_memory() -> Result<Self, async_sqlite::Error> {
        let client = ClientBuilder::new()
            .path(":memory:")
            .open()
            .await?;

        Self::init_schema(&client).await?;

        Ok(Self { client })
    }

    /// Initializes the cache table schema.
    async fn init_schema(client: &Client) -> Result<(), async_sqlite::Error> {
        client
            .conn(|conn| {
                conn.execute(
                    "CREATE TABLE IF NOT EXISTS cache (
                        key TEXT PRIMARY KEY,
                        data BLOB NOT NULL,
                        created_at INTEGER NOT NULL,
                        expires_at INTEGER NOT NULL
                    )",
                    [],
                )?;
                // Index for efficient GC queries
                conn.execute(
                    "CREATE INDEX IF NOT EXISTS idx_cache_expires_at ON cache(expires_at)",
                    [],
                )?;
                Ok(())
            })
            .await
    }

    /// Returns the number of entries in the cache (including expired ones).
    pub async fn len(&self) -> Result<usize, async_sqlite::Error> {
        self.client
            .conn(|conn| {
                conn.query_row("SELECT COUNT(*) FROM cache", [], |row| row.get::<_, i64>(0))
                    .map(|count| count as usize)
            })
            .await
    }

    /// Returns `true` if the cache is empty.
    pub async fn is_empty(&self) -> Result<bool, async_sqlite::Error> {
        self.len().await.map(|len| len == 0)
    }
}

#[async_trait]
impl CacheProvider for SqliteCache {
    async fn get(&self, key: &str) -> Option<CachedValue> {
        let key = key.to_string();
        let now = Utc::now().timestamp();

        let result = self
            .client
            .conn(move |conn| {
                conn.query_row(
                    "SELECT data, created_at, expires_at FROM cache WHERE key = ? AND expires_at > ?",
                    rusqlite::params![key, now],
                    |row| {
                        let data: Vec<u8> = row.get(0)?;
                        let created_at: i64 = row.get(1)?;
                        let expires_at: i64 = row.get(2)?;
                        Ok((data, created_at, expires_at))
                    },
                )
            })
            .await;

        match result {
            Ok((data, created_at, expires_at)) => {
                let created_at = Utc.timestamp_opt(created_at, 0).single()?;
                let expires_at = Utc.timestamp_opt(expires_at, 0).single()?;
                Some(CachedValue::new(data, created_at, expires_at))
            }
            Err(_) => None,
        }
    }

    async fn set(&self, key: &str, value: CachedValue) {
        let key = key.to_string();
        let data = value.data;
        let created_at = value.created_at.timestamp();
        let expires_at = value.expires_at.timestamp();

        let _ = self
            .client
            .conn(move |conn| {
                conn.execute(
                    "INSERT OR REPLACE INTO cache (key, data, created_at, expires_at) VALUES (?, ?, ?, ?)",
                    rusqlite::params![key, data, created_at, expires_at],
                )
            })
            .await;
    }

    async fn remove(&self, key: &str) {
        let key = key.to_string();

        let _ = self
            .client
            .conn(move |conn| conn.execute("DELETE FROM cache WHERE key = ?", [key]))
            .await;
    }

    async fn clear(&self) {
        let _ = self
            .client
            .conn(|conn| conn.execute("DELETE FROM cache", []))
            .await;
    }

    async fn gc(&self) -> usize {
        let now = Utc::now().timestamp();

        self.client
            .conn(move |conn| {
                conn.execute("DELETE FROM cache WHERE expires_at <= ?", [now])
                    .map(|count| count)
            })
            .await
            .unwrap_or(0)
    }
}
