//! Client management system for managing environments, accounts, and connections.

mod modal;
mod requests;

pub use requests::*;

use dataverse_lib::cache::SqliteCache;
use dataverse_lib::DataverseClient;
use modal::ClientManagementModal;
use rafter::prelude::*;

use crate::client_manager::{ClientManager, ClientManagerError};
use crate::credentials::CredentialsProvider;
use crate::paths;

/// System for managing Dataverse client connections.
#[system]
pub struct ClientManagement {
    manager: Option<ClientManager>,
}

#[system_impl]
impl ClientManagement {
    #[on_start]
    async fn on_start(&self, gx: &GlobalContext) {
        let credentials = gx.data::<CredentialsProvider>().clone();
        self.manager.set(Some(ClientManager::new(credentials)));
    }

    #[keybinds]
    fn keys() {
        bind("alt+m", open_client_management);
    }

    #[handler]
    async fn open_client_management(&self, gx: &GlobalContext) {
        let _result = gx.modal(ClientManagementModal::default()).await;
    }

    #[request_handler]
    async fn handle_get_active_client(
        &self,
        _request: GetActiveClient,
        gx: &GlobalContext,
    ) -> Result<ActiveClientInfo, ClientManagerError> {
        let manager = self
            .manager
            .get()
            .as_ref()
            .expect("ClientManager not initialized")
            .clone();

        let credentials = gx.data::<CredentialsProvider>();

        // Get active session
        let session = credentials.get_active_session().await?;
        let (account_id, env_id) = match (session.account_id, session.environment_id) {
            (Some(a), Some(e)) => (a, e),
            _ => return Err(ClientManagerError::NoActiveSession),
        };

        // Get account info
        let account = credentials
            .get_account(account_id)
            .await?
            .ok_or(ClientManagerError::NotFound {
                entity: "account",
                id: account_id,
            })?;

        // Get environment info
        let environment = credentials
            .get_environment(env_id)
            .await?
            .ok_or(ClientManagerError::NotFound {
                entity: "environment",
                id: env_id,
            })?;

        // Try to open persistent cache
        let cache = self.open_cache(&environment.url, gx).await;

        // Get or create client
        let client = manager.get_client(account_id, env_id, cache).await?;

        Ok(ActiveClientInfo {
            client,
            account_name: account.display_name,
            environment_name: environment.display_name,
            environment_url: environment.url,
        })
    }

    #[request_handler]
    async fn handle_get_client(
        &self,
        request: GetClient,
        gx: &GlobalContext,
    ) -> Result<ActiveClientInfo, ClientManagerError> {
        let manager = self
            .manager
            .get()
            .as_ref()
            .expect("ClientManager not initialized")
            .clone();

        let credentials = gx.data::<CredentialsProvider>();

        // Get account info
        let account = credentials
            .get_account(request.account_id)
            .await?
            .ok_or(ClientManagerError::NotFound {
                entity: "account",
                id: request.account_id,
            })?;

        // Get environment info
        let environment = credentials
            .get_environment(request.env_id)
            .await?
            .ok_or(ClientManagerError::NotFound {
                entity: "environment",
                id: request.env_id,
            })?;

        // Try to open persistent cache
        let cache = self.open_cache(&environment.url, gx).await;

        // Get or create client
        let client = manager
            .get_client(request.account_id, request.env_id, cache)
            .await?;

        Ok(ActiveClientInfo {
            client,
            account_name: account.display_name,
            environment_name: environment.display_name,
            environment_url: environment.url,
        })
    }

    /// Try to open a persistent SQLite cache for the given environment URL.
    /// Returns None and toasts a warning if the cache cannot be opened.
    async fn open_cache(&self, env_url: &str, gx: &GlobalContext) -> Option<SqliteCache> {
        let cache_path = paths::environment_cache_db(env_url)?;

        match SqliteCache::open(&cache_path).await {
            Ok(cache) => Some(cache),
            Err(e) => {
                let msg = format!(
                    "Failed to open cache at {}: {}. Using in-memory cache.",
                    cache_path.display(),
                    e
                );
                log::warn!("{}", msg);
                gx.toast(Toast::warning(msg));
                None
            }
        }
    }
}
