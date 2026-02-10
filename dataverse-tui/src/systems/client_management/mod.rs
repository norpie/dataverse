//! Client management system for managing environments, accounts, and connections.

mod modal;
mod requests;

pub use requests::*;

use std::sync::Arc;

use dataverse_lib::cache::SqliteCache;
use modal::ClientManagementModal;
use rafter::prelude::*;

use crate::client_manager::{ClientManager, ClientManagerError};
use crate::credentials::CredentialsProvider;
use crate::paths;

/// System for managing Dataverse client connections.
#[system]
pub struct ClientManagement {
    manager: Option<Arc<ClientManager>>,
}

#[system_impl]
impl ClientManagement {
    #[on_start]
    async fn on_start(&self, gx: &GlobalContext) {
        let credentials = gx.data::<CredentialsProvider>().clone();
        self.manager
            .set(Some(Arc::new(ClientManager::new(credentials))));

        // Publish ready event so other systems know ClientManagement is initialized
        let session = self.handle_get_active_session(GetActiveSession, gx).await;
        gx.publish(ClientManagementReady {
            session: session.clone(),
        });

        // Also publish initial session state
        if let Some(session) = session {
            gx.publish(SessionChanged {
                account_id: Some(session.account_id),
                env_id: Some(session.env_id),
                account_name: Some(session.account_name),
                environment_name: Some(session.environment_name),
                environment_url: Some(session.environment_url),
            });
        } else {
            gx.publish(SessionChanged {
                account_id: None,
                env_id: None,
                account_name: None,
                environment_name: None,
                environment_url: None,
            });
        }
    }

    #[keybinds]
    fn keys() {
        bind("alt+m", open_client_management);
    }

    #[handler]
    async fn open_client_management(&self, gx: &GlobalContext) {
        let _result = gx.modal(ClientManagementModal::default()).await;
    }

    #[event_handler]
    async fn on_open_client_management_modal(
        &self,
        _event: OpenClientManagementModal,
        gx: &GlobalContext,
    ) {
        let _result = gx.modal(ClientManagementModal::default()).await;
    }

    #[request_handler]
    async fn handle_get_active_client(
        &self,
        _request: GetActiveClient,
        gx: &GlobalContext,
    ) -> Result<ActiveClientInfo, ClientManagerError> {
        let manager_opt = self.manager.get();
        let Some(manager) = manager_opt.as_ref() else {
            log::error!("ClientManager not initialized when handling GetActiveClient");
            gx.toast(Toast::error("ClientManager not initialized"));
            return Err(ClientManagerError::NotInitialized);
        };
        let manager = Arc::clone(manager);

        let credentials = gx.data::<CredentialsProvider>();

        // Get active session
        let session = credentials.get_active_session().await?;
        let (account_id, env_id) = match (session.account_id, session.environment_id) {
            (Some(a), Some(e)) => (a, e),
            _ => return Err(ClientManagerError::NoActiveSession),
        };

        // Get account info
        let account =
            credentials
                .get_account(account_id)
                .await?
                .ok_or(ClientManagerError::NotFound {
                    entity: "account",
                    id: account_id,
                })?;

        // Get environment info
        let environment =
            credentials
                .get_environment(env_id)
                .await?
                .ok_or(ClientManagerError::NotFound {
                    entity: "environment",
                    id: env_id,
                })?;

        // Try to open persistent cache
        let cache = self.open_cache(&environment.url, gx).await;
        log::debug!(
            "open_cache returned: {}",
            if cache.is_some() {
                "Some(SqliteCache)"
            } else {
                "None"
            }
        );

        // Get or create client
        let client = manager.get_client(account_id, env_id, cache).await?;
        log::debug!("get_client returned client");

        Ok(ActiveClientInfo {
            client,
            account_id,
            env_id,
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
        let manager_opt = self.manager.get();
        let Some(manager) = manager_opt.as_ref() else {
            log::error!("ClientManager not initialized when handling GetClient");
            gx.toast(Toast::error("ClientManager not initialized"));
            return Err(ClientManagerError::NotInitialized);
        };
        let manager = manager.clone();

        let credentials = gx.data::<CredentialsProvider>();

        // Get account info
        let account = credentials.get_account(request.account_id).await?.ok_or(
            ClientManagerError::NotFound {
                entity: "account",
                id: request.account_id,
            },
        )?;

        // Get environment info
        let environment = credentials.get_environment(request.env_id).await?.ok_or(
            ClientManagerError::NotFound {
                entity: "environment",
                id: request.env_id,
            },
        )?;

        // Try to open persistent cache
        let cache = self.open_cache(&environment.url, gx).await;

        // Get or create client
        let client = manager
            .get_client(request.account_id, request.env_id, cache)
            .await?;

        Ok(ActiveClientInfo {
            client,
            account_id: request.account_id,
            env_id: request.env_id,
            account_name: account.display_name,
            environment_name: environment.display_name,
            environment_url: environment.url,
        })
    }

    #[request_handler]
    async fn handle_get_any_client(
        &self,
        request: GetAnyClient,
        gx: &GlobalContext,
    ) -> Result<ActiveClientInfo, ClientManagerError> {
        let credentials = gx.data::<CredentialsProvider>();

        // Find any account with tokens for this environment
        let pairs = credentials.list_authenticated_pairs().await?;
        let account_id = pairs
            .iter()
            .find(|(_, env_id)| *env_id == request.env_id)
            .map(|(account_id, _)| *account_id)
            .ok_or(ClientManagerError::NoAuthenticatedAccount {
                env_id: request.env_id,
            })?;

        // Delegate to GetClient handler
        self.handle_get_client(
            GetClient {
                account_id,
                env_id: request.env_id,
            },
            gx,
        )
        .await
    }

    #[request_handler]
    async fn handle_get_authenticated_environments(
        &self,
        _request: GetAuthenticatedEnvironments,
        gx: &GlobalContext,
    ) -> Vec<AuthenticatedEnvironment> {
        let credentials = gx.data::<CredentialsProvider>();

        // Get all (account_id, env_id) pairs with tokens
        let pairs = match credentials.list_authenticated_pairs().await {
            Ok(pairs) => pairs,
            Err(e) => {
                log::error!("Failed to list authenticated pairs: {}", e);
                return vec![];
            }
        };

        // Determine the active session's account for deduplication preference
        let active_account_id = credentials
            .get_active_session()
            .await
            .ok()
            .and_then(|s| s.account_id);

        // Deduplicate by environment: prefer active session account, else lowest ID
        let mut env_map: std::collections::HashMap<i64, (i64, i64)> =
            std::collections::HashMap::new();
        for (account_id, env_id) in pairs {
            env_map
                .entry(env_id)
                .and_modify(|existing| {
                    let prefer_new = active_account_id == Some(account_id)
                        && active_account_id != Some(existing.0);
                    let lower_id = account_id < existing.0 && active_account_id != Some(existing.0);
                    if prefer_new || lower_id {
                        *existing = (account_id, env_id);
                    }
                })
                .or_insert((account_id, env_id));
        }

        // Resolve full info for each unique pair
        let mut result = Vec::with_capacity(env_map.len());
        for (env_id, (account_id, _)) in env_map {
            let account = match credentials.get_account(account_id).await {
                Ok(Some(a)) => a,
                _ => continue,
            };
            let environment = match credentials.get_environment(env_id).await {
                Ok(Some(e)) => e,
                _ => continue,
            };
            result.push(AuthenticatedEnvironment {
                account_id,
                env_id,
                account_name: account.display_name,
                environment_name: environment.display_name,
                environment_url: environment.url,
            });
        }

        // Sort by environment name
        result.sort_by(|a, b| a.environment_name.cmp(&b.environment_name));
        result
    }

    #[request_handler]
    async fn handle_get_active_session(
        &self,
        _request: GetActiveSession,
        gx: &GlobalContext,
    ) -> Option<SessionInfo> {
        let credentials = gx.data::<CredentialsProvider>();

        let session = credentials.get_active_session().await.ok()?;
        let (account_id, env_id) = match (session.account_id, session.environment_id) {
            (Some(a), Some(e)) => (a, e),
            _ => return None,
        };

        let account = credentials.get_account(account_id).await.ok()??;
        let environment = credentials.get_environment(env_id).await.ok()??;

        Some(SessionInfo {
            account_id,
            env_id,
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
