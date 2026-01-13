//! Setup system for initial account configuration.

use rafter::prelude::*;

use crate::client_manager::ClientManager;

/// System that triggers initial setup when no accounts exist.
#[system]
pub struct SetupSystem;

#[system_impl]
impl SetupSystem {
    #[on_start]
    async fn check_setup(&self, gx: &GlobalContext) {
        let client_manager = gx.data::<ClientManager>();
        if !client_manager.has_accounts().await {
            // TODO: Launch SetupModal instead
            gx.toast(Toast::warning("No accounts configured. Setup modal should be triggered."));
        }
    }
}
