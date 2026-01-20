//! Launcher system for opening apps.

mod modal;

use rafter::prelude::*;

use crate::apps::EntityExplorer;
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
            match selected.as_str() {
                "entity-explorer" => {
                    let _ = gx.spawn_and_focus(EntityExplorer::default());
                }
                _ => {
                    gx.toast(Toast::info(format!("App not implemented: {}", selected)));
                }
            }
        }
    }
}
