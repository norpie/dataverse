//! Migration SQLite repository.

use async_sqlite::Client;
use async_sqlite::ClientBuilder;
use std::path::Path;
use thiserror::Error;

// Re-export input types
pub use coalesce_chain::NewCoalesceChain;
pub use entity_mapping::{NewEntityMapping, UpdateEntityMapping};
pub use field_mapping::NewFieldMapping;
pub use find_condition::{NewFindCondition, UpdateFindCondition};
pub use match_condition::{NewMatchCondition, UpdateMatchCondition};
pub use migration::NewMigration;
pub use phase::{NewPhase, UpdatePhase};
pub use phase_run::NewPhaseRun;
pub use transform::{NewMatchBranch, NewTransform, UpdateMatchBranch, UpdateTransform};
pub use variable::{NewVariable, UpdateVariable};

/// Semantic update type for nullable fields.
#[derive(Debug, Clone)]
pub enum Update<T> {
    /// Don't change the field.
    Keep,
    /// Set the field to a new value.
    Set(T),
    /// Clear the field (set to NULL).
    Clear,
}

// Internal modules
mod coalesce_chain;
mod entity_mapping;
mod field_mapping;
mod find_condition;
mod helpers;
mod match_condition;
mod migration;
mod phase;
mod phase_run;
mod transform;
mod variable;

/// Errors from repository operations.
#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error("Database error: {0}")]
    Database(#[from] async_sqlite::Error),
    #[error("Migration error: {0}")]
    Migration(#[from] crate::migrations::MigrationError),
    #[error("Serialization error: {0}")]
    Serialization(String),
    #[error("Deserialization error: {0}")]
    Deserialization(String),
    #[error("Not found: {0} with id {1}")]
    NotFound(&'static str, i64),
    #[error("Invalid enum value: {0}")]
    InvalidEnum(String),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

impl From<bincode::error::EncodeError> for RepositoryError {
    fn from(e: bincode::error::EncodeError) -> Self {
        RepositoryError::Serialization(e.to_string())
    }
}

impl From<bincode::error::DecodeError> for RepositoryError {
    fn from(e: bincode::error::DecodeError) -> Self {
        RepositoryError::Deserialization(e.to_string())
    }
}

/// Repository for migration configuration data.
#[derive(Clone)]
pub struct MigrationRepository {
    client: Client,
}

impl MigrationRepository {
    /// Create a new repository, initializing the database schema.
    pub async fn new(path: &Path) -> Result<Self, RepositoryError> {
        let client = ClientBuilder::new().path(path).open().await?;

        // Run migrations
        let migrations = super::migrations::load()?;
        crate::migrations::run(&client, &migrations).await?;

        Ok(Self { client })
    }
}
