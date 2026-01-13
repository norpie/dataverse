//! Client manager for Dataverse clients.
//!
//! Provides shared rate limiting and caching for multiple Dataverse clients.

use dashmap::DashMap;
use dataverse_lib::rate_limit::RateLimiter;
use dataverse_lib::DataverseClient;
use thiserror::Error;

use crate::credentials::{
    Account, CredentialsError, CredentialsProvider, Environment, StoredTokenProvider,
};

/// Error type for client manager operations.
#[derive(Debug, Error)]
pub enum ClientManagerError {
    #[error("credentials error: {0}")]
    Credentials(#[from] CredentialsError),

    #[error("authentication error: {0}")]
    Auth(#[from] dataverse_lib::error::AuthError),

    #[error("API error: {0}")]
    Api(#[from] dataverse_lib::error::Error),

    #[error("{entity} not found: {id}")]
    NotFound { entity: &'static str, id: i64 },

    #[error("no active session")]
    NoActiveSession,
}

/// Manages Dataverse clients with shared rate limiting and caching.
///
/// Clients are cached by (account_id, env_id) and share a rate limiter
/// to respect Dataverse's service protection limits.
#[derive(Clone)]
pub struct ClientManager {
    credentials: CredentialsProvider,
    rate_limiter: RateLimiter,
    clients: DashMap<(i64, i64), DataverseClient>,
}

impl ClientManager {
    /// Creates a new client manager.
    pub fn new(credentials: CredentialsProvider) -> Self {
        Self {
            credentials,
            rate_limiter: RateLimiter::default(),
            clients: DashMap::new(),
        }
    }

    /// Returns true if any accounts exist in the credentials store.
    pub async fn has_accounts(&self) -> bool {
        match self.credentials.list_accounts().await {
            Ok(accounts) => !accounts.is_empty(),
            Err(_) => false,
        }
    }

    /// Returns true if there is an active session.
    pub async fn has_active_session(&self) -> bool {
        match self.credentials.get_active_session().await {
            Ok(session) => session.is_active(),
            Err(_) => false,
        }
    }

    /// Gets the client for the active session.
    ///
    /// Returns `None` if there is no active session.
    pub async fn get_active_client(&self) -> Result<Option<DataverseClient>, ClientManagerError> {
        let session = self.credentials.get_active_session().await?;

        let (account_id, env_id) = match (session.account_id, session.environment_id) {
            (Some(a), Some(e)) => (a, e),
            _ => return Ok(None),
        };

        let client = self.get_client(account_id, env_id).await?;
        Ok(Some(client))
    }

    /// Gets or creates a client for the given account and environment.
    ///
    /// Clients are cached and reused. The first call for a given account/environment
    /// will create the client and verify connectivity via WhoAmI.
    pub async fn get_client(
        &self,
        account_id: i64,
        env_id: i64,
    ) -> Result<DataverseClient, ClientManagerError> {
        let key = (account_id, env_id);

        // Check cache first
        if let Some(client) = self.clients.get(&key) {
            return Ok(client.clone());
        }

        // Get account and environment
        let account = self
            .credentials
            .get_account(account_id)
            .await?
            .ok_or(ClientManagerError::NotFound {
                entity: "account",
                id: account_id,
            })?;

        let environment = self
            .credentials
            .get_environment(env_id)
            .await?
            .ok_or(ClientManagerError::NotFound {
                entity: "environment",
                id: env_id,
            })?;

        // Create client
        let client = self.create_client(account, environment).await?;

        // Cache and return
        self.clients.insert(key, client.clone());
        Ok(client)
    }

    /// Invalidates the cached client for the given account and environment.
    pub fn invalidate_client(&self, account_id: i64, env_id: i64) {
        self.clients.remove(&(account_id, env_id));
    }

    /// Invalidates all cached clients.
    pub fn invalidate_all(&self) {
        self.clients.clear();
    }

    /// Creates a new client for the given account and environment.
    async fn create_client(
        &self,
        account: Account,
        environment: Environment,
    ) -> Result<DataverseClient, ClientManagerError> {
        let token_provider =
            StoredTokenProvider::new(self.credentials.clone(), account, environment.clone());

        let client = DataverseClient::builder()
            .url(&environment.url)
            .token_provider(token_provider)
            .shared_rate_limiter(self.rate_limiter.clone())
            .build();

        // Verify connectivity
        client.connect().await?;

        Ok(client)
    }
}
