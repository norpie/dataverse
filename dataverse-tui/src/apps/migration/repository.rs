//! Migration SQLite repository.

use async_sqlite::Client;
use async_sqlite::ClientBuilder;
use chrono::DateTime;
use chrono::Utc;
use std::path::Path;
use thiserror::Error;

use super::types::*;
use crate::widgets::filter_builder::FilterNode;

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
