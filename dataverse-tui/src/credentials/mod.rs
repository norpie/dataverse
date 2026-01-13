//! Credentials storage system for OAuth tokens and account management.

mod models;
mod sqlite;
mod token_provider;

pub use models::Account;
pub use models::ActiveSession;
pub use models::AuthType;
pub use models::CachedTokens;
pub use models::Environment;
pub use sqlite::SqliteCredentialsBackend;
pub use token_provider::StoredTokenProvider;

use std::sync::Arc;

use async_trait::async_trait;
use dataverse_lib::error::AuthError;
use thiserror::Error;

/// Credentials error type.
#[derive(Debug, Error)]
pub enum CredentialsError {
    #[error("database error: {0}")]
    Database(#[from] async_sqlite::Error),
    #[error("{entity} with id {id} not found")]
    NotFound { entity: &'static str, id: i64 },
    #[error("re-authentication required for account {account_id} on environment {environment_id}")]
    ReauthRequired { account_id: i64, environment_id: i64 },
    #[error("authentication error: {0}")]
    Auth(#[from] AuthError),
    #[error("invalid auth type: {0}")]
    InvalidAuthType(String),
}

/// Backend trait for credentials storage.
#[async_trait]
pub trait CredentialsBackend: Send + Sync {
    // Environments
    async fn create_environment(
        &self,
        url: &str,
        display_name: &str,
    ) -> Result<Environment, CredentialsError>;
    async fn get_environment(&self, id: i64) -> Result<Option<Environment>, CredentialsError>;
    async fn get_environment_by_url(&self, url: &str)
        -> Result<Option<Environment>, CredentialsError>;
    async fn list_environments(&self) -> Result<Vec<Environment>, CredentialsError>;
    async fn update_environment(
        &self,
        id: i64,
        url: &str,
        display_name: &str,
    ) -> Result<(), CredentialsError>;
    async fn delete_environment(&self, id: i64) -> Result<(), CredentialsError>;

    // Accounts
    async fn create_account(&self, account: &Account) -> Result<Account, CredentialsError>;
    async fn get_account(&self, id: i64) -> Result<Option<Account>, CredentialsError>;
    async fn list_accounts(&self) -> Result<Vec<Account>, CredentialsError>;
    async fn update_account(&self, account: &Account) -> Result<(), CredentialsError>;
    async fn delete_account(&self, id: i64) -> Result<(), CredentialsError>;

    // Tokens
    async fn get_tokens(
        &self,
        account_id: i64,
        env_id: i64,
    ) -> Result<Option<CachedTokens>, CredentialsError>;
    async fn save_tokens(
        &self,
        account_id: i64,
        env_id: i64,
        tokens: &CachedTokens,
    ) -> Result<(), CredentialsError>;
    async fn clear_tokens(&self, account_id: i64, env_id: i64) -> Result<(), CredentialsError>;

    // Active session
    async fn get_active_session(&self) -> Result<ActiveSession, CredentialsError>;
    async fn set_active_session(
        &self,
        account_id: Option<i64>,
        env_id: Option<i64>,
    ) -> Result<(), CredentialsError>;
}

/// Credentials provider for managing accounts, environments, and tokens.
#[derive(Clone)]
pub struct CredentialsProvider {
    backend: Arc<dyn CredentialsBackend>,
}

impl CredentialsProvider {
    /// Create a new credentials provider with the given backend.
    pub fn new(backend: impl CredentialsBackend + 'static) -> Self {
        Self {
            backend: Arc::new(backend),
        }
    }

    // =========================================================================
    // Environments
    // =========================================================================

    /// Create a new environment.
    pub async fn create_environment(
        &self,
        url: &str,
        display_name: &str,
    ) -> Result<Environment, CredentialsError> {
        self.backend.create_environment(url, display_name).await
    }

    /// Get an environment by ID.
    pub async fn get_environment(&self, id: i64) -> Result<Option<Environment>, CredentialsError> {
        self.backend.get_environment(id).await
    }

    /// Get an environment by URL.
    pub async fn get_environment_by_url(
        &self,
        url: &str,
    ) -> Result<Option<Environment>, CredentialsError> {
        self.backend.get_environment_by_url(url).await
    }

    /// List all environments.
    pub async fn list_environments(&self) -> Result<Vec<Environment>, CredentialsError> {
        self.backend.list_environments().await
    }

    /// Update an environment.
    pub async fn update_environment(
        &self,
        id: i64,
        url: &str,
        display_name: &str,
    ) -> Result<(), CredentialsError> {
        self.backend.update_environment(id, url, display_name).await
    }

    /// Delete an environment.
    pub async fn delete_environment(&self, id: i64) -> Result<(), CredentialsError> {
        self.backend.delete_environment(id).await
    }

    // =========================================================================
    // Accounts
    // =========================================================================

    /// Create a new account.
    pub async fn create_account(&self, account: &Account) -> Result<Account, CredentialsError> {
        self.backend.create_account(account).await
    }

    /// Get an account by ID.
    pub async fn get_account(&self, id: i64) -> Result<Option<Account>, CredentialsError> {
        self.backend.get_account(id).await
    }

    /// List all accounts.
    pub async fn list_accounts(&self) -> Result<Vec<Account>, CredentialsError> {
        self.backend.list_accounts().await
    }

    /// Update an account.
    pub async fn update_account(&self, account: &Account) -> Result<(), CredentialsError> {
        self.backend.update_account(account).await
    }

    /// Delete an account.
    pub async fn delete_account(&self, id: i64) -> Result<(), CredentialsError> {
        self.backend.delete_account(id).await
    }

    // =========================================================================
    // Tokens
    // =========================================================================

    /// Get cached tokens for an account-environment pair.
    pub async fn get_tokens(
        &self,
        account_id: i64,
        env_id: i64,
    ) -> Result<Option<CachedTokens>, CredentialsError> {
        self.backend.get_tokens(account_id, env_id).await
    }

    /// Save tokens for an account-environment pair.
    pub async fn save_tokens(
        &self,
        account_id: i64,
        env_id: i64,
        tokens: &CachedTokens,
    ) -> Result<(), CredentialsError> {
        self.backend.save_tokens(account_id, env_id, tokens).await
    }

    /// Clear tokens for an account-environment pair.
    pub async fn clear_tokens(&self, account_id: i64, env_id: i64) -> Result<(), CredentialsError> {
        self.backend.clear_tokens(account_id, env_id).await
    }

    // =========================================================================
    // Active Session
    // =========================================================================

    /// Get the active session.
    pub async fn get_active_session(&self) -> Result<ActiveSession, CredentialsError> {
        self.backend.get_active_session().await
    }

    /// Set the active session.
    pub async fn set_active_session(
        &self,
        account_id: Option<i64>,
        env_id: Option<i64>,
    ) -> Result<(), CredentialsError> {
        self.backend.set_active_session(account_id, env_id).await
    }

    // =========================================================================
    // Token Provider Factory
    // =========================================================================

    /// Create a token provider for the given account and environment.
    pub async fn get_token_provider(
        &self,
        account_id: i64,
        env_id: i64,
    ) -> Result<StoredTokenProvider, CredentialsError> {
        let account = self
            .get_account(account_id)
            .await?
            .ok_or(CredentialsError::NotFound {
                entity: "account",
                id: account_id,
            })?;

        let environment = self
            .get_environment(env_id)
            .await?
            .ok_or(CredentialsError::NotFound {
                entity: "environment",
                id: env_id,
            })?;

        Ok(StoredTokenProvider::new(
            self.clone(),
            account,
            environment,
        ))
    }
}
