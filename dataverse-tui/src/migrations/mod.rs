//! Database migration framework.
//!
//! Each module with a database has its own `migrations/` subdirectory containing:
//! - `mod.rs` with `include_dir!` to embed migrations at compile time
//! - `NNN_name.sql` files for each migration
//!
//! Usage:
//! ```ignore
//! // In credentials/migrations/mod.rs:
//! use include_dir::{Dir, include_dir};
//! static DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/src/credentials/migrations");
//! pub fn load() -> Vec<Migration> { crate::migrations::load_from_dir(&DIR) }
//!
//! // To run migrations:
//! let migrations = credentials::migrations::load();
//! crate::migrations::run(&client, &migrations).await?;
//! ```

use std::collections::BTreeMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::Hash;
use std::hash::Hasher;

use async_sqlite::Client;
use include_dir::Dir;
use thiserror::Error;

/// A single migration.
#[derive(Debug, Clone)]
pub struct Migration {
    /// Version number (from filename prefix).
    pub version: i64,
    /// Name (from filename after prefix).
    pub name: String,
    /// SQL to execute.
    pub sql: String,
}

/// A migration that has been applied.
#[derive(Debug, Clone)]
pub struct AppliedMigration {
    pub version: i64,
    pub name: String,
    pub applied_at: String,
    pub checksum: String,
}

/// Migration errors.
#[derive(Debug, Error)]
pub enum MigrationError {
    #[error("Database error: {0}")]
    Database(#[from] async_sqlite::Error),
    #[error("No migrations found")]
    NoMigrations,
    #[error("Invalid migration filename: {0}")]
    InvalidFilename(String),
    #[error("Migration {version} checksum mismatch: expected {expected}, found {found}")]
    ChecksumMismatch {
        version: i64,
        expected: String,
        found: String,
    },
    #[error("Applied migration {version} '{name}' not found in available migrations")]
    MissingMigration { version: i64, name: String },
}

/// Load migrations from an embedded directory.
///
/// Expects files named `NNN_name.sql` where NNN is a zero-padded version number.
pub fn load_from_dir(dir: &Dir<'_>) -> Result<Vec<Migration>, MigrationError> {
    let mut migrations = BTreeMap::new();

    for file in dir.files() {
        let path = file.path();
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| MigrationError::InvalidFilename(format!("{:?}", path)))?;

        // Skip non-SQL files and mod.rs
        if !filename.ends_with(".sql") {
            continue;
        }

        // Parse NNN_name.sql
        let stem = filename.trim_end_matches(".sql");
        let parts: Vec<&str> = stem.splitn(2, '_').collect();
        if parts.len() != 2 {
            return Err(MigrationError::InvalidFilename(filename.to_string()));
        }

        let version: i64 = parts[0]
            .parse()
            .map_err(|_| MigrationError::InvalidFilename(filename.to_string()))?;
        let name = parts[1].to_string();

        let sql = file
            .contents_utf8()
            .ok_or_else(|| MigrationError::InvalidFilename(filename.to_string()))?
            .to_string();

        migrations.insert(version, Migration { version, name, sql });
    }

    if migrations.is_empty() {
        return Err(MigrationError::NoMigrations);
    }

    // Return sorted by version
    Ok(migrations.into_values().collect())
}

/// Calculate checksum for migration SQL.
///
/// Normalizes line endings for cross-platform consistency.
pub fn checksum(sql: &str) -> String {
    let normalized = sql.replace("\r\n", "\n").replace('\r', "\n");
    let mut hasher = DefaultHasher::new();
    normalized.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

/// Run pending migrations on a database.
pub async fn run(client: &Client, migrations: &[Migration]) -> Result<(), MigrationError> {
    // Initialize tracking table
    client
        .conn(|conn| {
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS schema_migrations (
                    version INTEGER PRIMARY KEY,
                    name TEXT NOT NULL,
                    applied_at TEXT NOT NULL DEFAULT (datetime('now')),
                    checksum TEXT NOT NULL
                )",
            )?;
            Ok(())
        })
        .await?;

    // Get applied migrations
    let applied = get_applied(client).await?;

    // Validate existing migrations haven't changed
    for applied_migration in &applied {
        let available = migrations
            .iter()
            .find(|m| m.version == applied_migration.version);

        match available {
            Some(migration) => {
                let expected = checksum(&migration.sql);
                if applied_migration.checksum != expected {
                    return Err(MigrationError::ChecksumMismatch {
                        version: applied_migration.version,
                        expected,
                        found: applied_migration.checksum.clone(),
                    });
                }
            }
            None => {
                return Err(MigrationError::MissingMigration {
                    version: applied_migration.version,
                    name: applied_migration.name.clone(),
                });
            }
        }
    }

    // Apply pending migrations
    let applied_versions: std::collections::HashSet<i64> =
        applied.iter().map(|m| m.version).collect();

    for migration in migrations {
        if applied_versions.contains(&migration.version) {
            continue;
        }

        log::info!(
            "Applying migration {} '{}'",
            migration.version,
            migration.name
        );

        let sql = migration.sql.clone();
        let version = migration.version;
        let name = migration.name.clone();
        let hash = checksum(&migration.sql);

        client
            .conn_mut(move |conn| {
                // Disable foreign keys before the transaction so that
                // table rebuilds (DROP + RENAME) don't trigger CASCADE deletes.
                // PRAGMA foreign_keys is a no-op inside transactions.
                conn.execute_batch("PRAGMA foreign_keys = OFF;")?;

                // Run in a transaction for atomicity
                let tx = conn.transaction()?;

                // Execute migration
                tx.execute_batch(&sql)?;

                // Record it
                tx.execute(
                    "INSERT INTO schema_migrations (version, name, checksum) VALUES (?1, ?2, ?3)",
                    rusqlite::params![version, name, hash],
                )?;

                tx.commit()?;

                // Re-enable foreign keys
                conn.execute_batch("PRAGMA foreign_keys = ON;")?;
                Ok(())
            })
            .await?;

        log::info!("Migration {} applied", migration.version);
    }

    Ok(())
}

/// Get applied migrations from a database.
pub async fn get_applied(client: &Client) -> Result<Vec<AppliedMigration>, MigrationError> {
    // Check if table exists first
    let exists: bool = client
        .conn(|conn| {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='schema_migrations'",
                [],
                |row| row.get(0),
            )?;
            Ok(count > 0)
        })
        .await?;

    if !exists {
        return Ok(vec![]);
    }

    let migrations = client
        .conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT version, name, applied_at, checksum FROM schema_migrations ORDER BY version",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok(AppliedMigration {
                    version: row.get(0)?,
                    name: row.get(1)?,
                    applied_at: row.get(2)?,
                    checksum: row.get(3)?,
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>()
        })
        .await?;

    Ok(migrations)
}
