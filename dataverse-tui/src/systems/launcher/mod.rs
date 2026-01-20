//! Launcher system for opening apps.

mod modal;

use rafter::prelude::*;

use modal::LauncherModal;

#[system]
pub struct Launcher;

#[system_impl]
impl Launcher {
    #[keybinds]
    fn keys() {
        bind("ctrl+p", open_launcher);
    }

    #[handler]
    async fn open_launcher(&self, gx: &GlobalContext) {
        let result = gx.modal(LauncherModal::default()).await;

        if let Some(selected) = result {
            // Entity Browser removed - will be replaced by Entity Explorer in Phase 2
            gx.toast(Toast::info(format!("App not implemented: {}", selected)));
        }
    }
}
