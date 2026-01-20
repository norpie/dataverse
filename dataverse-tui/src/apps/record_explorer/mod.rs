//! Record Explorer app for viewing Dataverse entity records.

mod row;
mod service;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use dataverse_lib::DataverseClient;
use dataverse_lib::api::query::odata::ODataPages;
use rafter::EventData;
use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{
    Autocomplete, AutocompleteState, Column, SelectionMode, Table, TableRow, TableState, Text,
};
use tuidom::Element;

use crate::formatting::default_column_width;
use crate::systems::client_management::{ClientManagement, GetActiveClient};
use crate::widgets::{Spinner, loading_overlay};

use row::{EntityData, RecordRow};
use service::{
    build_field_options, default_columns, fetch_entity_data, fetch_next_page, fetch_records,
};

#[app(name = "Record Explorer")]
pub struct RecordExplorer {
    // Entity to explore (set at construction, never changes)
    #[state(skip)]
    entity_logical_name: String,

    // Connection info (set once on startup)
    client: Option<DataverseClient>,
    environment_name: String,

    // Loading overlay message (None = no overlay)
    loading_message: Option<String>,

    // Entity data (Some after initial load)
    entity_data: Option<EntityData>,
    field_autocomplete: AutocompleteState<String>,

    // Records table
    records: TableState<RecordRow>,
    records_loading: Resource<()>,
    pages: Option<ODataPages>,
    total_count: Option<usize>,

    // Advanced mode - shared with rows for efficient rendering
    #[state(skip)]
    advanced_mode: Arc<AtomicBool>,
}

impl RecordExplorer {
    pub fn new(entity_logical_name: impl Into<String>) -> Self {
        Self {
            entity_logical_name: entity_logical_name.into(),
            ..Default::default()
        }
    }
}

#[app_impl]
impl RecordExplorer {
    #[on_start]
    async fn on_start(&self, gx: &GlobalContext, cx: &AppContext) {
        self.loading_message
            .set(Some("Connecting to Dataverse...".to_string()));

        // Get client info once
        let info = match gx
            .request_system::<ClientManagement, GetActiveClient>(GetActiveClient)
            .await
        {
            Ok(Ok(info)) => info,
            Ok(Err(e)) => {
                gx.toast(Toast::error(format!("Client error: {}", e)));
                self.loading_message.set(None);
                return;
            }
            Err(e) => {
                gx.toast(Toast::error(format!(
                    "No active client. Please configure a connection first. ({:?})",
                    e
                )));
                self.loading_message.set(None);
                return;
            }
        };

        // Store client and environment info
        self.client.set(Some(info.client.clone()));
        self.environment_name.set(info.environment_name);

        // Load the entity specified at construction
        self.loading_message
            .set(Some(format!("Loading {}...", self.entity_logical_name)));
        self.load_entity(&self.entity_logical_name, gx).await;

        self.loading_message.set(None);
        cx.focus("field-autocomplete");
    }

    fn title(&self) -> String {
        let env_name = self.environment_name.get();
        let entity_name = self.entity_data.with_ref(|data| {
            data.as_ref()
                .and_then(|d| d.metadata.display_name.text())
                .unwrap_or(&self.entity_logical_name)
                .to_string()
        });

        format!("{} ({})", entity_name, env_name)
    }

    /// Get the stored client.
    fn client(&self) -> Option<DataverseClient> {
        self.client.get()
    }

    #[keybinds]
    fn keybinds() {
        bind("r", refresh);
        bind("f2", toggle_advanced);
    }

    #[handler]
    async fn refresh(&self, gx: &GlobalContext, _cx: &AppContext) {
        let display_name = self.entity_data.with_ref(|data| {
            data.as_ref()
                .and_then(|d| d.metadata.display_name.text())
                .map(|s| s.to_string())
                .unwrap_or_else(|| self.entity_logical_name.clone())
        });

        self.loading_message
            .set(Some(format!("Refreshing {}...", display_name)));

        // Preserve current field selection
        let selected_fields: Vec<String> = self
            .field_autocomplete
            .get()
            .selected_values()
            .cloned()
            .collect();

        // Reload entity and records
        self.load_entity(&self.entity_logical_name, gx).await;

        // Restore field selection if any were selected
        if !selected_fields.is_empty() {
            self.field_autocomplete.update(|state| {
                state.selection.selected.clear();
                for field in &selected_fields {
                    state.selection.selected.insert(field.clone());
                }
            });

            // Reload records with the restored field selection
            self.load_records_for_fields(&selected_fields, gx).await;
        }

        self.loading_message.set(None);
        gx.toast(Toast::info("Refreshed"));
    }

    #[handler]
    async fn toggle_advanced(&self, gx: &GlobalContext, _cx: &AppContext) {
        let new_mode = !self.advanced_mode.load(Ordering::Relaxed);
        self.advanced_mode.store(new_mode, Ordering::Relaxed);

        // Swap field autocomplete labels
        if let Some(entity_data) = self.entity_data.get() {
            self.field_autocomplete.update(|state| {
                for (key, label) in &mut state.options {
                    if new_mode {
                        *label = key.clone();
                    } else if let Some(attr) = entity_data
                        .readable_fields
                        .iter()
                        .find(|a| &a.logical_name == key)
                    {
                        *label = attr.display_name.text().unwrap_or(key).to_string();
                    }
                }
            });

            // Swap table column headers
            self.records.update(|state| {
                for col in &mut state.columns {
                    if new_mode {
                        col.header = col.id.clone();
                    } else if let Some(attr) = entity_data
                        .readable_fields
                        .iter()
                        .find(|a| a.logical_name == col.id)
                    {
                        col.header = attr.display_name.text().unwrap_or(&col.id).to_string();
                    }
                }
            });
        }

        // Row cell values automatically pick up new mode on next render (via Arc<AtomicBool>)
        gx.toast(Toast::info(if new_mode {
            "Advanced mode"
        } else {
            "Normal mode"
        }));
    }

    /// Load an entity's metadata, set up field autocomplete, and load initial records.
    async fn load_entity(&self, logical_name: &str, gx: &GlobalContext) {
        let client = match self.client() {
            Some(c) => c,
            None => {
                gx.toast(Toast::error("Client connection lost"));
                return;
            }
        };

        // Fetch entity metadata
        let entity_data = match fetch_entity_data(&client, logical_name).await {
            Ok(data) => data,
            Err(e) => {
                gx.toast(Toast::error(format!("Failed to load entity: {}", e)));
                return;
            }
        };

        // Build field autocomplete options
        let is_advanced = self.advanced_mode.load(Ordering::Relaxed);
        let field_options = build_field_options(&entity_data, is_advanced);
        self.field_autocomplete
            .set(AutocompleteState::new(field_options).with_selection(SelectionMode::Multi));

        // Store entity data
        self.entity_data.set(Some(entity_data.clone()));

        // Load initial records with default columns
        let columns = default_columns(&entity_data.metadata);
        self.do_load_records(&client, &entity_data, &columns, gx)
            .await;
    }

    #[handler]
    async fn on_field_change(&self, gx: &GlobalContext, _cx: &AppContext) {
        let state = self.field_autocomplete.get();
        let selected_fields: Vec<String> = state.selected_values().cloned().collect();

        self.load_records_for_fields(&selected_fields, gx).await;
    }

    /// Load records for the given field selection.
    async fn load_records_for_fields(&self, selected_fields: &[String], gx: &GlobalContext) {
        let client = match self.client() {
            Some(c) => c,
            None => {
                gx.toast(Toast::error("Client connection lost"));
                return;
            }
        };

        let entity_data = match self.entity_data.get() {
            Some(data) => data,
            None => return,
        };

        let columns: Vec<String> = if selected_fields.is_empty() {
            default_columns(&entity_data.metadata)
        } else {
            selected_fields.to_vec()
        };

        self.do_load_records(&client, &entity_data, &columns, gx)
            .await;
    }

    /// Internal method to load records with given columns.
    async fn do_load_records(
        &self,
        client: &DataverseClient,
        entity_data: &EntityData,
        columns: &[String],
        gx: &GlobalContext,
    ) {
        // Don't start a new fetch if already loading
        if self.records_loading.is_loading() {
            return;
        }

        self.records_loading.set_loading();
        self.total_count.set(None);

        // Build table columns
        let is_advanced = self.advanced_mode.load(Ordering::Relaxed);
        let table_columns: Vec<Column> = columns
            .iter()
            .map(|col_name| {
                let attr = entity_data
                    .readable_fields
                    .iter()
                    .find(|a| &a.logical_name == col_name);

                let header = if is_advanced {
                    col_name.clone()
                } else {
                    attr.and_then(|a| a.display_name.text())
                        .unwrap_or(col_name)
                        .to_string()
                };

                let width = attr
                    .map(|a| default_column_width(&a.attribute_type))
                    .unwrap_or(20);

                Column::new(col_name, &header).fixed(width)
            })
            .collect();

        // Fetch records
        let result = match fetch_records(
            client,
            &entity_data.metadata,
            columns,
            50,
            self.advanced_mode.clone(),
        )
        .await
        {
            Ok(r) => r,
            Err(e) => {
                log::error!("[EntityBrowser] Failed to fetch records: {}", e);
                self.records_loading
                    .set_error(format!("Failed to load records: {}", e));
                self.pages.set(None);
                return;
            }
        };

        if let Some(count) = result.total_count {
            log::debug!("[EntityBrowser] Total count: {}", count);
            self.total_count.set(Some(count));
        }

        self.pages.set(result.pages);

        // Update table
        let frozen_col = table_columns.first().map(|c| c.id.clone());
        let mut state =
            TableState::new(result.rows, table_columns).with_selection(SelectionMode::None);
        if let Some(col) = &frozen_col {
            state = state.with_frozen(&[col.as_str()]);
        }
        self.records.set(state);

        self.records_loading.set_ready(());
    }

    #[handler]
    async fn on_table_scroll(&self, gx: &GlobalContext, _cx: &AppContext, event: &EventData) {
        // Check if near bottom (80% scrolled)
        if event.is_near_bottom(0.8) {
            self.load_more_records(gx).await;
        }
    }

    async fn load_more_records(&self, gx: &GlobalContext) {
        // Don't load more if already loading
        if self.records_loading.is_loading() {
            return;
        }

        // Get pages iterator
        let mut pages: ODataPages = match self.pages.get() {
            Some(p) => p,
            None => return,
        };

        let entity_data = match self.entity_data.get() {
            Some(data) => data,
            None => return,
        };

        self.records_loading.set_progress(ProgressState {
            current: 0,
            total: None,
            message: Some("Loading more records...".to_string()),
        });

        // Fetch next page
        let (new_rows, has_more) = match fetch_next_page(
            &mut pages,
            &entity_data.metadata.primary_id_attribute,
            self.advanced_mode.clone(),
        )
        .await
        {
            Ok(Some((rows, has_more))) => (rows, has_more),
            Ok(None) => {
                self.pages.set(None);
                self.records_loading.set_ready(());
                return;
            }
            Err(e) => {
                log::error!("[EntityBrowser] Failed to fetch more records: {}", e);
                gx.toast(Toast::error("Failed to load more records"));
                self.pages.set(None);
                self.records_loading.set_ready(());
                return;
            }
        };

        self.records.update(|state| {
            state.extend_rows(new_rows);
        });

        // Store pages iterator back (if more pages exist)
        if has_more {
            self.pages.set(Some(pages));
        } else {
            self.pages.set(None);
        }

        self.records_loading.set_ready(());
    }

    fn element(&self) -> Element {
        let loading_message = self.loading_message.get();
        // Extract only needed values without cloning all rows
        let (has_records, loaded_count, column_count) = self
            .records
            .with_ref(|t| (!t.rows.is_empty(), t.rows.len(), t.columns.len()));
        let records_state = self.records_loading.get();
        let total_count = self.total_count.get();

        page! {
            box_ (width: fill, height: fill) {
                // Main content
                column (padding: (1, 2), gap: 1, height: fill, width: fill) style (bg: background) {
                    // Header row with field selection
                    row (gap: 2) {
                        column (gap: 0) {
                            text (content: "Fields") style (fg: muted)
                            autocomplete (
                                state: self.field_autocomplete,
                                id: "field-autocomplete",
                                placeholder: "Select fields...",
                                width: 40
                            )
                                on_change: on_field_change()
                        }
                    }

                    // Table area
                    if has_records {
                        box_ (id: "table-container", height: fill, width: fill) style (bg: surface) {
                            table (state: self.records, id: "records-table")
                                on_scroll: on_table_scroll()
                        }
                    } else {
                        match records_state {
                            ResourceState::Loading => {
                                column (height: fill, width: fill, align: center, justify: center) style (bg: surface) {
                                    spinner (id: "table-spinner")
                                }
                            }
                            ResourceState::Error(ref e) => {
                                column (height: fill, width: fill, align: center, justify: center) style (bg: surface) {
                                    text (content: {e.to_string()}) style (fg: error)
                                }
                            }
                            _ => {
                                box_ (height: fill, width: fill) style (bg: surface) {}
                            }
                        }
                    }

                    // Footer row: loaded/total | spinner | columns (outside if block like table example)
                    row (width: fill, justify: between) {
                        // Left: record count
                        if let Some(total) = total_count {
                            text (content: {format!("{}/{}", loaded_count, total)}) style (fg: muted)
                        } else {
                            text (content: {format!("{} records", loaded_count)}) style (fg: muted)
                        }

                        // Center: pagination spinner
                        match records_state {
                            ResourceState::Progress(_) => {
                                spinner (id: "pagination-spinner")
                            }
                            _ => {}
                        }

                        // Right: column count
                        text (content: {format!("{} columns", column_count)}) style (fg: muted)
                    }
                }

                // Loading overlay (shown during initial load or entity switch)
                if let Some(msg) = loading_message {
                    { loading_overlay("loading-overlay", &msg) }
                }
            }
        }
    }
}
