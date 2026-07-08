//! Audit Log app: view the change history (audit trail) for a single record.

mod row;
mod select_modal;

pub use row::AuditRow;

use dataverse_lib::DataverseClient;
use dataverse_lib::error::Error as DataverseError;
use dataverse_lib::model::Entity;
use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{Column, SelectionMode, Table, TableState, Text};
use tuidom::Element;
use uuid::Uuid;

use crate::modals::LoadingModal;
use crate::systems::client_management::ActiveClientInfo;
use crate::widgets::Spinner;

use row::{
    COL_ACTION, COL_ATTRIBUTE, COL_NEW, COL_OLD, COL_OPERATION, COL_TIMESTAMP, COL_USER,
    flatten_history,
};
use select_modal::AuditSelectModal;

/// The record whose audit history is being viewed.
#[derive(Clone, Debug)]
pub struct AuditTarget {
    pub logical_name: String,
    pub id: Uuid,
}

#[app(name = "Audit Log")]
pub struct AuditLog {
    #[state(skip)]
    client_info: ActiveClientInfo,

    /// Selected record (set once the user confirms the selection modal).
    target: Option<AuditTarget>,

    /// Audit rows table.
    rows: TableState<AuditRow>,
    loading: Resource<()>,
}

impl AuditLog {
    /// Create the Audit Log app with the given client.
    pub fn with_client(client_info: ActiveClientInfo) -> Self {
        Self::new(client_info, None, TableState::default())
    }
}

/// Fetch all entities as sorted `(logical, display)` pairs.
async fn fetch_entities(client: &DataverseClient) -> Result<Vec<(String, String)>, DataverseError> {
    let all = client.metadata().all_entities().await?;
    let mut entities: Vec<(String, String)> = all
        .iter()
        .map(|e| {
            let display = e.display_name.text().unwrap_or(&e.logical_name).to_string();
            (e.logical_name.clone(), display)
        })
        .collect();
    entities.sort_by(|a, b| a.1.to_lowercase().cmp(&b.1.to_lowercase()));
    Ok(entities)
}

#[app_impl]
impl AuditLog {
    #[on_start]
    async fn on_start(&self, gx: &GlobalContext, cx: &AppContext) {
        // Load the entity list for the selection modal.
        let client = self.client_info.client.clone();
        let entities = match gx
            .modal(LoadingModal::run_with_default(
                "Loading entities...",
                || Err(DataverseError::Cancelled),
                async move { fetch_entities(&client).await },
            ))
            .await
        {
            Ok(entities) => entities,
            Err(e) if e.is_cancelled() => {
                cx.close();
                return;
            }
            Err(e) => {
                gx.toast(Toast::error(format!("Failed to load entities: {}", e)));
                cx.close();
                return;
            }
        };

        // Ask the user for the entity + record id.
        let target = match gx.modal(AuditSelectModal::with_entities(entities)).await {
            Some(target) => target,
            None => {
                cx.close();
                return;
            }
        };

        self.target.set(Some(target));
        self.load_history(gx).await;
    }

    fn title(&self) -> String {
        let suffix = self
            .target
            .with_ref(|t| {
                t.as_ref()
                    .map(|t| format!(" — {} {}", t.logical_name, t.id))
            })
            .unwrap_or_default();
        format!(
            "Audit Log ({}){}",
            self.client_info.environment_name, suffix
        )
    }

    #[keybinds]
    fn keybinds() {
        bind("r", refresh);
        bind("escape", close_app);
    }

    #[handler]
    async fn close_app(&self, cx: &AppContext) {
        cx.close();
    }

    #[handler]
    async fn refresh(&self, gx: &GlobalContext) {
        self.load_history(gx).await;
        gx.toast(Toast::info("Refreshed"));
    }

    /// Fetch the change history for the current target and populate the table.
    async fn load_history(&self, gx: &GlobalContext) {
        let target = match self.target.get() {
            Some(t) => t,
            None => return,
        };

        if self.loading.is_loading() {
            return;
        }
        self.loading.set_loading();

        let client = self.client_info.client.clone();
        let entity = Entity::logical(&target.logical_name);
        let id = target.id;
        let result = gx
            .modal(LoadingModal::run_with_default(
                "Loading audit history...",
                || Err(DataverseError::Cancelled),
                async move { client.retrieve_record_change_history(&entity, id).await },
            ))
            .await;

        let collection = match result {
            Ok(collection) => collection,
            Err(e) if e.is_cancelled() => {
                self.loading.set_ready(());
                return;
            }
            Err(e) => {
                self.loading
                    .set_error(format!("Failed to load audit history: {}", e));
                return;
            }
        };

        let rows = flatten_history(&collection);
        let columns = vec![
            Column::new(COL_TIMESTAMP, "Timestamp").fixed(24),
            Column::new(COL_ACTION, "Action").fixed(8),
            Column::new(COL_OPERATION, "Operation").fixed(10),
            Column::new(COL_USER, "User").fixed(38),
            Column::new(COL_ATTRIBUTE, "Attribute").fixed(28),
            Column::new(COL_OLD, "Old value").fixed(32),
            Column::new(COL_NEW, "New value").fixed(32),
        ];

        let state = TableState::new(rows, columns)
            .with_selection(SelectionMode::None)
            .with_frozen(&[COL_TIMESTAMP]);
        self.rows.set(state);

        self.loading.set_ready(());
    }

    fn element(&self) -> Element {
        let (has_rows, row_count) = self.rows.with_ref(|t| (!t.rows.is_empty(), t.rows.len()));
        let state = self.loading.get();

        page! {
            column (padding: (1, 2), gap: 1, height: fill, width: fill) style (bg: background) {
                if has_rows {
                    box_ (id: "audit-table-container", height: fill, width: fill) style (bg: surface) {
                        table (state: self.rows, id: "audit-table")
                    }
                } else {
                    match state {
                        ResourceState::Loading => {
                            column (height: fill, width: fill, align: center, justify: center) style (bg: surface) {
                                spinner (id: "audit-spinner")
                            }
                        }
                        ResourceState::Error(ref e) => {
                            column (height: fill, width: fill, align: center, justify: center) style (bg: surface) {
                                text (content: {e.to_string()}) style (fg: error)
                            }
                        }
                        _ => {
                            column (height: fill, width: fill, align: center, justify: center) style (bg: surface) {
                                text (content: "No audit records") style (fg: muted)
                            }
                        }
                    }
                }

                row (width: fill, justify: between) {
                    text (content: {format!("{} changes", row_count)}) style (fg: muted)
                    row (gap: 1) {
                        text (content: "r") style (fg: primary)
                        text (content: "refresh") style (fg: muted)
                    }
                }
            }
        }
    }
}
