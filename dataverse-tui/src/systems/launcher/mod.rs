//! Launcher system for opening apps.

use rafter::prelude::*;

use crate::apps::{
    DeadlineImport, EntityExplorer, Import, MigrationList, QueryBuilder, QuestionnaireSync,
    QuestionnaireValidator,
};
use crate::modals::{ListEntry, SearchableListModal};
use crate::systems::client_management::{ClientManagement, GetActiveClient};

#[system]
pub struct Launcher {
    /// Lock to prevent multiple launcher modals from being opened simultaneously.
    modal_open: bool,
}

#[system_impl]
impl Launcher {
    #[keybinds]
    fn keys() {
        bind("ctrl+p", open_launcher);
    }

    #[handler]
    async fn open_launcher(&self, gx: &GlobalContext) {
        // Check if modal is already open
        if self.modal_open.get() {
            return;
        }

        // Set lock
        self.modal_open.set(true);

        let items = vec![
            ListEntry::with_category("entity-explorer", "Entity Explorer", "Data"),
            ListEntry::with_category("query-builder", "Query Builder", "Tools"),
            ListEntry::with_category("import", "Import", "Tools"),
            ListEntry::with_category("migrations", "Migrations", "Data"),
            ListEntry::with_category("deadline-import", "VAF - Deadline Import", "Data"),
            ListEntry::with_category("questionnaire-sync", "VAF - Questionnaire Sync", "Data"),
            ListEntry::with_category(
                "questionnaire-validator",
                "VAF - Questionnaire Validator",
                "Data",
            ),
        ];

        let result = gx
            .modal(SearchableListModal::with_entries("Launcher", items))
            .await;

        // Clear lock
        self.modal_open.set(false);

        if let Some(selected) = result {
            match selected.as_str() {
                // Apps that don't need a client
                "migrations" => {
                    let _ = gx.spawn_and_focus(MigrationList::create());
                }
                // Apps that need a client
                app => {
                    let client_info = match gx
                        .request_system::<ClientManagement, GetActiveClient>(GetActiveClient)
                        .await
                    {
                        Ok(Ok(info)) => info,
                        Ok(Err(e)) => {
                            gx.toast(Toast::error(format!("Client error: {}", e)));
                            return;
                        }
                        Err(e) => {
                            gx.toast(Toast::error(format!(
                                "No active client. Please configure a connection first. ({:?})",
                                e
                            )));
                            return;
                        }
                    };
                    match app {
                        "entity-explorer" => {
                            let _ = gx.spawn_and_focus(EntityExplorer::with_client(client_info));
                        }
                        "query-builder" => {
                            let _ = gx.spawn_and_focus(QueryBuilder::with_client(client_info));
                        }
                        "deadline-import" => {
                            let _ = gx.spawn_and_focus(DeadlineImport::with_client(client_info));
                        }
                        "questionnaire-sync" => {
                            let _ = gx.spawn_and_focus(QuestionnaireSync::create());
                        }
                        "questionnaire-validator" => {
                            let _ = gx.spawn_and_focus(QuestionnaireValidator::create());
                        }
                        "import" => {
                            let _ = gx.spawn_and_focus(Import::with_client(client_info));
                        }
                        _ => {
                            gx.toast(Toast::info(format!("App not implemented: {}", app)));
                        }
                    }
                }
            }
        }
    }
}
