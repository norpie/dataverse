//! Launcher system for opening apps.

use rafter::prelude::*;

use crate::apps::{EntityExplorer, QueryBuilder};
use crate::modals::{ListEntry, SearchableListModal};

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
        let items = vec![
            ListEntry::with_category("entity-explorer", "Entity Explorer", "Data"),
            ListEntry::with_category("query-builder", "Query Builder", "Tools"),
        ];

        let result = gx.modal(SearchableListModal::new("Launcher", items)).await;

        if let Some(selected) = result {
            match selected.as_str() {
                "entity-explorer" => {
                    let _ = gx.spawn_and_focus(EntityExplorer::default());
                }
                "query-builder" => {
                    let _ = gx.spawn_and_focus(QueryBuilder::default());
                }
                _ => {
                    gx.toast(Toast::info(format!("App not implemented: {}", selected)));
                }
            }
        }
    }
}
