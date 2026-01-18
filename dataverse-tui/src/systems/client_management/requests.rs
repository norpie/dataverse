//! Request types for client management.

use dataverse_lib::DataverseClient;
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
