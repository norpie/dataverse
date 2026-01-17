//! Entity Browser app for viewing Dataverse entity records.

use std::collections::HashMap;

use dataverse_lib::api::query::odata::ODataPages;
use dataverse_lib::model::metadata::{AttributeMetadata, EntityMetadata};
use dataverse_lib::model::{Entity, Value};
use dataverse_lib::DataverseClient;
use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{
    Autocomplete, AutocompleteState, Column, SelectionMode, Table, TableRow, TableState, Text,
};
use rafter::EventData;
use tuidom::Element;

use crate::widgets::{loading_overlay, Spinner};
use crate::ClientManager;

/// Entity data including metadata and readable fields.
#[derive(Clone, Debug)]
pub struct EntityData {
    pub metadata: EntityMetadata,
    pub readable_fields: Vec<AttributeMetadata>,
}

/// A record row for the table.
#[derive(Clone, Debug)]
pub struct RecordRow {
    id: String,
    cells: HashMap<String, String>,
}

impl RecordRow {
    fn new(id: String) -> Self {
        Self {
            id,
            cells: HashMap::new(),
        }
    }

    fn set_cell(&mut self, column: String, value: String) {
        self.cells.insert(column, value);
    }
}

impl TableRow for RecordRow {
    type Key = String;

    fn key(&self) -> String {
        self.id.clone()
    }

    fn cell(&self, column_id: &str) -> Element {
        let text = self.cells.get(column_id).cloned().unwrap_or_default();
        Element::text(&text)
    }
}

#[app(name = "Entity Browser")]
pub struct EntityBrowser {
    // Loading overlay message (None = no overlay)
    loading_message: Option<String>,

    // Entity selection
    entities: Vec<(String, String)>,
    entity_autocomplete: AutocompleteState<String>,

    // Entity data (Some after initial load)
    entity_data: Option<EntityData>,
    field_autocomplete: AutocompleteState<String>,

    // Records table
    records: TableState<RecordRow>,
    records_loading: Resource<()>,
    pages: Option<ODataPages>,
    total_count: Option<usize>,
}

#[app_impl]
impl EntityBrowser {
    #[on_start]
    async fn on_start(&self, gx: &GlobalContext, cx: &AppContext) {
        self.loading_message
            .set(Some("Loading Dataverse metadata...".to_string()));

        // Get client
        let client = match get_client(gx).await {
            Some(c) => c,
            None => {
                gx.toast(Toast::error(
                    "No active client. Please configure a connection first.",
                ));
                self.loading_message.set(None);
                return;
            }
        };

        // Load all entities
        let all_entities = match client.metadata().all_entities().await {
            Ok(entities) => entities,
            Err(e) => {
                gx.toast(Toast::error(format!("Failed to load entities: {}", e)));
                self.loading_message.set(None);
                return;
            }
        };

        // Build autocomplete options: (logical_name, display_name)
        let mut options: Vec<(String, String)> = all_entities
            .iter()
            .map(|e| {
                let display = e
                    .display_name
                    .text()
                    .unwrap_or(&e.core.logical_name)
                    .to_string();
                (e.core.logical_name.clone(), display)
            })
            .collect();

        // Sort by display name
        options.sort_by(|a, b| a.1.to_lowercase().cmp(&b.1.to_lowercase()));

        self.entities.set(options.clone());
        self.entity_autocomplete.set(AutocompleteState::new(options));

        // Auto-select: contact > account > first available
        let auto_select = all_entities
            .iter()
            .find(|e| e.core.logical_name == "contact")
            .or_else(|| all_entities.iter().find(|e| e.core.logical_name == "account"))
            .or_else(|| all_entities.first());

        let selected_logical_name = match auto_select {
            Some(entity) => entity.core.logical_name.clone(),
            None => {
                gx.toast(Toast::error("No entities available"));
                self.loading_message.set(None);
                return;
            }
        };

        // Load the selected entity's full metadata
        self.load_entity_data(gx, cx, &client, &selected_logical_name)
            .await;

        self.loading_message.set(None);
        cx.focus("field-autocomplete");
    }

    fn title(&self) -> String {
        match self.entity_data.get() {
            Some(data) => {
                let name = data
                    .metadata
                    .display_name
                    .text()
                    .unwrap_or(&data.metadata.core.logical_name);
                format!("Entity Browser - {}", name)
            }
            None => "Entity Browser".to_string(),
        }
    }

    #[handler]
    async fn on_entity_select(&self, gx: &GlobalContext, cx: &AppContext) {
        let state = self.entity_autocomplete.get();
        let selected_key = match state.value() {
            Some(key) => key.clone(),
            None => return,
        };

        // Get display name for loading message
        let display_name = self
            .entities
            .get()
            .iter()
            .find(|(k, _)| k == &selected_key)
            .map(|(_, d)| d.clone())
            .unwrap_or_else(|| selected_key.clone());

        log::debug!("[EntityBrowser] Entity selected: {}", selected_key);
        self.loading_message
            .set(Some(format!("Loading {}...", display_name)));

        let client = match get_client(gx).await {
            Some(c) => c,
            None => {
                gx.toast(Toast::error("No active client"));
                self.loading_message.set(None);
                return;
            }
        };

        self.load_entity_data(gx, cx, &client, &selected_key).await;

        self.loading_message.set(None);
        cx.focus("field-autocomplete");
    }

    async fn load_entity_data(
        &self,
        gx: &GlobalContext,
        _cx: &AppContext,
        client: &DataverseClient,
        logical_name: &str,
    ) {
        // Load full entity metadata (with attributes)
        let entity = match client.metadata().entity(logical_name).await {
            Ok(e) => e,
            Err(e) => {
                gx.toast(Toast::error(format!("Failed to load entity: {}", e)));
                return;
            }
        };

        // Filter readable attributes for field selection
        let readable_attrs: Vec<AttributeMetadata> = entity
            .attributes
            .iter()
            .filter(|a| a.is_valid_for_read && a.attribute_of.is_none())
            .cloned()
            .collect();

        // Build field autocomplete options
        let mut field_options: Vec<(String, String)> = readable_attrs
            .iter()
            .map(|a| {
                let display = a.display_name.text().unwrap_or(&a.logical_name).to_string();
                (a.logical_name.clone(), display)
            })
            .collect();

        field_options.sort_by(|a, b| a.1.to_lowercase().cmp(&b.1.to_lowercase()));

        // Set up field autocomplete with multi-select
        self.field_autocomplete.set(
            AutocompleteState::new(field_options).with_selection(SelectionMode::Multi),
        );

        // Update entity data
        let entity_data = EntityData {
            metadata: entity.clone(),
            readable_fields: readable_attrs,
        };
        self.entity_data.set(Some(entity_data));

        // Update autocomplete selection to show current entity
        self.entity_autocomplete.update(|state| {
            // Find the label for this logical name
            if let Some((_, label)) = state.options.iter().find(|(k, _)| k == logical_name) {
                state.text = label.clone();
            }
            state.selection.selected.clear();
            state.selection.selected.insert(logical_name.to_string());
        });

        // Load initial records with default columns
        self.load_records(gx, &entity, Vec::new()).await;
    }

    #[handler]
    async fn on_field_change(&self, gx: &GlobalContext) {
        let entity_data = match self.entity_data.get() {
            Some(data) => data,
            None => return,
        };

        let state = self.field_autocomplete.get();
        let selected_fields: Vec<String> = state.selected_values().cloned().collect();

        self.load_records(gx, &entity_data.metadata, selected_fields)
            .await;
    }

    #[handler]
    async fn on_table_scroll(&self, gx: &GlobalContext, event: &EventData) {
        // Check if near bottom (80% scrolled)
        if event.is_near_bottom(0.8) {
            self.load_more_records(gx).await;
        }
    }

    async fn load_records(
        &self,
        gx: &GlobalContext,
        entity: &EntityMetadata,
        selected_fields: Vec<String>,
    ) {
        // Don't start a new fetch if already loading
        if self.records_loading.is_loading() {
            return;
        }

        self.records_loading.set_loading();

        let client = match get_client(gx).await {
            Some(c) => c,
            None => {
                self.records_loading.set_error("No active client");
                return;
            }
        };

        // Determine columns to fetch
        let columns: Vec<String> = if selected_fields.is_empty() {
            // Default: primary name + createdon + modifiedon
            let mut cols = Vec::new();
            if let Some(primary) = &entity.core.primary_name_attribute {
                cols.push(primary.clone());
            }
            cols.push("createdon".to_string());
            cols.push("modifiedon".to_string());
            cols
        } else {
            selected_fields
        };

        // Build table columns
        let entity_data = self.entity_data.get();
        let available = entity_data
            .as_ref()
            .map(|d| d.readable_fields.clone())
            .unwrap_or_default();

        let table_columns: Vec<Column> = columns
            .iter()
            .map(|col_name| {
                let display = available
                    .iter()
                    .find(|a| &a.logical_name == col_name)
                    .and_then(|a| a.display_name.text())
                    .unwrap_or(col_name)
                    .to_string();

                Column::new(col_name, &display).fixed(20)
            })
            .collect();

        // Build select list including ID
        let mut select_cols: Vec<&str> = columns.iter().map(|s| s.as_str()).collect();
        select_cols.push(&entity.core.primary_id_attribute);

        // Reset total count for new query
        self.total_count.set(None);

        // Create query
        let query = client
            .query(Entity::Set(entity.core.entity_set_name.clone()))
            .select(&select_cols)
            .page_size(50);

        // Run count query and first page fetch in parallel
        let count_query = query.clone();
        let (count_result, mut pages) = tokio::join!(
            count_query.count(),
            async { query.into_async_iter() }
        );

        // Store total count if successful
        match count_result {
            Ok(count) => {
                log::debug!("[EntityBrowser] Total count: {}", count);
                self.total_count.set(Some(count));
            }
            Err(e) => {
                log::error!("[EntityBrowser] Count query failed: {}", e);
                gx.toast(Toast::error(format!("Failed to get record count: {}", e)));
            }
        }
        let page = match pages.next().await {
            Some(Ok(p)) => p,
            Some(Err(e)) => {
                log::error!("[EntityBrowser] Failed to fetch records: {}", e);
                self.records_loading
                    .set_error(format!("Failed to load records: {}", e));
                self.pages.set(None);
                return;
            }
            None => {
                let frozen_col = table_columns.first().map(|c| c.id.clone());
                let mut state =
                    TableState::new(Vec::new(), table_columns).with_selection(SelectionMode::None);
                if let Some(col) = &frozen_col {
                    state = state.with_frozen(&[col.as_str()]);
                }
                self.records.set(state);
                self.pages.set(None);
                self.records_loading.set_ready(());
                return;
            }
        };

        // Convert records to table rows
        let rows = convert_records_to_rows(page.records(), &entity.core.primary_id_attribute);

        // Store pages iterator for pagination (if more pages exist)
        if page.has_more() {
            self.pages.set(Some(pages));
        } else {
            self.pages.set(None);
        }

        // Update table
        let frozen_col = table_columns.first().map(|c| c.id.clone());
        let mut state =
            TableState::new(rows, table_columns).with_selection(SelectionMode::None);
        if let Some(col) = &frozen_col {
            state = state.with_frozen(&[col.as_str()]);
        }
        self.records.set(state);

        self.records_loading.set_ready(());
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
        let page: dataverse_lib::api::query::Page = match pages.next().await {
            Some(Ok(p)) => p,
            Some(Err(e)) => {
                log::error!("[EntityBrowser] Failed to fetch more records: {}", e);
                gx.toast(Toast::error("Failed to load more records"));
                self.pages.set(None);
                self.records_loading.set_ready(());
                return;
            }
            None => {
                self.pages.set(None);
                self.records_loading.set_ready(());
                return;
            }
        };

        // Convert and append records
        let new_rows = convert_records_to_rows(
            page.records(),
            &entity_data.metadata.core.primary_id_attribute,
        );

        self.records.update(|state| {
            // Must use set_rows() to rebuild cumulative_heights cache and update scroll content height
            let mut all_rows = std::mem::take(&mut state.rows);
            all_rows.extend(new_rows);
            state.set_rows(all_rows);
        });

        // Store pages iterator back (if more pages exist)
        if page.has_more() {
            self.pages.set(Some(pages));
        } else {
            self.pages.set(None);
        }

        self.records_loading.set_ready(());
    }

    fn element(&self) -> Element {
        let loading_message = self.loading_message.get();
        let has_entity = self.entity_data.get().is_some();
        let records_table = self.records.get();
        let has_records = !records_table.rows.is_empty();
        let records_state = self.records_loading.get();
        let total_count = self.total_count.get();
        let loaded_count = records_table.rows.len();
        let column_count = records_table.columns.len();

        page! {
            box_ (width: fill, height: fill) {
                // Main content
                column (padding: (1, 2), gap: 1, height: fill, width: fill) style (bg: background) {
                    // Header row with entity and field selection
                    row (gap: 2) {
                        column (gap: 0) {
                            text (content: "Entity") style (fg: muted)
                            autocomplete (
                                state: self.entity_autocomplete,
                                id: "entity-autocomplete",
                                placeholder: "Search entities...",
                                width: 30
                            )
                                on_select: on_entity_select()
                        }

                        if has_entity {
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
                    }

                    // Table area
                    if has_entity {
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

/// Helper to get the active client.
async fn get_client(gx: &GlobalContext) -> Option<DataverseClient> {
    let client_manager = gx.data::<ClientManager>();
    match client_manager.get_active_client().await {
        Ok(Some(client)) => Some(client),
        Ok(None) => None,
        Err(e) => {
            log::error!("[EntityBrowser] Failed to get client: {}", e);
            None
        }
    }
}

/// Convert dataverse records to table rows.
fn convert_records_to_rows(
    records: &[dataverse_lib::model::Record],
    id_attribute: &str,
) -> Vec<RecordRow> {
    records
        .iter()
        .enumerate()
        .map(|(idx, record)| {
            let id = record
                .id()
                .map(|u| u.to_string())
                .or_else(|| {
                    record
                        .get_guid(id_attribute)
                        .ok()
                        .flatten()
                        .map(|u| u.to_string())
                })
                .unwrap_or_else(|| format!("unknown-{}", idx));

            let mut row = RecordRow::new(id);

            // Populate cells - prefer formatted values
            for (key, _value) in record.fields() {
                let formatted = record
                    .get_formatted(key)
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| {
                        record
                            .get(key)
                            .map(|v| format_value(v))
                            .unwrap_or_default()
                    });
                row.set_cell(key.clone(), formatted);
            }

            row
        })
        .collect()
}

/// Format a Value for display.
fn format_value(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Bool(b) => if *b { "Yes" } else { "No" }.to_string(),
        Value::Int(n) => n.to_string(),
        Value::Long(n) => n.to_string(),
        Value::Float(n) => format!("{:.2}", n),
        Value::Decimal(d) => d.to_string(),
        Value::String(s) => s.clone(),
        Value::Guid(g) => g.to_string(),
        Value::DateTime(dt) => dt.format("%Y-%m-%d %H:%M").to_string(),
        Value::Money(m) => format!("{}", m.value()),
        Value::EntityReference(r) => r.name.clone().unwrap_or_else(|| r.id.to_string()),
        Value::OptionSet(o) => o.label.clone().unwrap_or_else(|| o.value.to_string()),
        Value::MultiOptionSet(o) => o
            .labels
            .as_ref()
            .map(|labels| labels.join(", "))
            .unwrap_or_else(|| {
                o.values
                    .iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            }),
        _ => "[complex]".to_string(),
    }
}
