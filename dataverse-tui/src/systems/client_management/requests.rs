//! Request and event types for client management.

use dataverse_lib::DataverseClient;
use rafter::Event;
use rafter::Request;

use crate::client_manager::ClientManagerError;

/// Information about the active client connection.
#[derive(Clone)]
pub struct ActiveClientInfo {
    /// The Dataverse client.
    pub client: DataverseClient,
    /// Account ID.
    pub account_id: i64,
    /// Environment ID.
    pub env_id: i64,
    /// Account display name.
    pub account_name: String,
    /// Environment display name.
    pub environment_name: String,
    /// Environment URL.
    pub environment_url: String,
}

/// Request to get the active client with connection info.
#[derive(Request)]
#[response(Result<ActiveClientInfo, ClientManagerError>)]
pub struct GetActiveClient;

/// Request to get a client for a specific account/environment.
#[derive(Request)]
#[response(Result<ActiveClientInfo, ClientManagerError>)]
pub struct GetClient {
    pub account_id: i64,
    pub env_id: i64,
}

/// Request to get a client for an environment using any available account.
#[derive(Request)]
#[response(Result<ActiveClientInfo, ClientManagerError>)]
pub struct GetAnyClient {
    pub env_id: i64,
}

/// Request to get all currently cached clients (no new clients created).
#[derive(Request)]
#[response(Vec<DataverseClient>)]
pub struct GetAllCachedClients;

/// Request to get the current active session info (lightweight, no client creation).
#[derive(Request)]
#[response(Option<SessionInfo>)]
pub struct GetActiveSession;

/// Current session information (without creating a client).
#[derive(Clone)]
pub struct SessionInfo {
    pub account_id: i64,
    pub env_id: i64,
    pub account_name: String,
    pub environment_name: String,
    pub environment_url: String,
}

/// Information about an environment with a valid authenticated account.
#[derive(Clone)]
pub struct AuthenticatedEnvironment {
    /// Account ID used for this environment.
    pub account_id: i64,
    /// Environment ID.
    pub env_id: i64,
    /// Account display name.
    pub account_name: String,
    /// Environment display name.
    pub environment_name: String,
    /// Environment URL.
    pub environment_url: String,
}

/// Request to get all environments that have at least one authenticated account.
///
/// Deduplicates by environment (prefers the active session's account).
/// Returns sorted by environment name.
#[derive(Request)]
#[response(Vec<AuthenticatedEnvironment>)]
pub struct GetAuthenticatedEnvironments;

// =============================================================================
// Events
// =============================================================================

/// Event to request opening the client management modal.
#[derive(Clone, Event)]
pub struct OpenClientManagementModal;

/// Event published once when ClientManagement has finished initializing.
/// Systems that depend on ClientManagement should use this instead of
/// requesting data in their own `on_start`.
#[derive(Clone, Event)]
pub struct ClientManagementReady {
    pub session: Option<SessionInfo>,
}

/// Event published when the active session changes (connect/disconnect).
#[derive(Clone, Event)]
pub struct SessionChanged {
    pub account_id: Option<i64>,
    pub env_id: Option<i64>,
    pub account_name: Option<String>,
    pub environment_name: Option<String>,
    pub environment_url: Option<String>,
}

/// Event published when an environment is added.
#[derive(Clone, Event)]
pub struct EnvironmentAdded {
    pub id: i64,
    pub url: String,
    pub display_name: String,
}

/// Event published when an environment is removed.
#[derive(Clone, Event)]
pub struct EnvironmentRemoved {
    pub id: i64,
}
