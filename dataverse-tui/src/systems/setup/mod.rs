//! Setup system for initial account configuration.

mod modal;

use rafter::prelude::*;

use crate::client_manager::ClientManager;
use modal::SetupModal;

/// System that triggers initial setup when no accounts exist.
#[system]
pub struct SetupSystem;

#[system_impl]
impl SetupSystem {
    #[on_start]
    async fn check_setup(&self, gx: &GlobalContext) {
        let client_manager = gx.data::<ClientManager>();
        if !client_manager.has_accounts().await {
            let _result = gx.modal(SetupModal::default()).await;
        }
    }
}
