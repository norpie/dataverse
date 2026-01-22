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

// =============================================================================
// Events
// =============================================================================

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
