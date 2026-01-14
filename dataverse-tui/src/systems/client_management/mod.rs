//! Client management system for managing environments, accounts, and connections.

mod modal;

use modal::ClientManagementModal;
use rafter::prelude::*;

/// System for managing Dataverse client connections.
#[system]
pub struct ClientManagement;

#[system_impl]
impl ClientManagement {
    #[keybinds]
    fn keys() {
        bind("alt+m", open_client_management);
    }

    #[handler]
    async fn open_client_management(&self, gx: &GlobalContext) {
        let _result = gx.modal(ClientManagementModal::default()).await;
    }
}
