//! Export app for exporting Dataverse query results to CSV or Excel.

use dataverse_lib::api::query::odata::ODataPages;
use dataverse_lib::model::{Entity, Record};
use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{Button, Column, SelectionMode, Table, TableState, Text};
use rafter::InstanceId;

use crate::apps::record_explorer::{RecordRow, convert_records_to_rows};
use crate::modals::{FileBrowserModal, LoadingModal};
use crate::paths;
use crate::systems::client_management::ActiveClientInfo;
use crate::widgets::Spinner;

/// Export app: execute query and export results to file.
#[app(name = "Export")]
pub struct Export {
    /// Query builder template for fetching records.
    #[state(skip)]
    query: dataverse_lib::api::query::odata::QueryBuilder,

    /// Full connection context.
    #[state(skip)]
    client_info: ActiveClientInfo,

    /// Optional origin instance (for back navigation).
    #[state(skip)]
    origin: Option<InstanceId>,

    /// Entity being queried.
    #[state(skip)]
    entity: Entity,

    /// Selected fields from query (empty = all fields).
    #[state(skip)]
    selected_fields: Vec<String>,

    /// All fetched records.
    records: Vec<Record>,

    /// Column names for display.
    columns: Vec<String>,

    /// Preview table state.
    preview_table: TableState<RecordRow>,

    /// Total record count.
    total_count: Option<usize>,
}

impl Export {
    pub fn new(
        query: dataverse_lib::api::query::odata::QueryBuilder,
        client_info: ActiveClientInfo,
        origin: Option<InstanceId>,
    ) -> Self {
        let entity = query.entity().clone();
        let selected_fields = query.selected_fields().to_vec();

        Self {
            query,
            client_info,
            origin,
            entity,
            selected_fields,
            records: State::default(),
            columns: State::default(),
            preview_table: State::default(),
            total_count: State::default(),
            __handler_registry: Default::default(),
            __derived_cache: Default::default(),
        }
    }
}

#[app_impl]
impl Export {
    #[on_start]
    async fn on_start(&self, gx: &GlobalContext, cx: &AppContext) {
        // Get the true record count using $apply=aggregate
        let client = self.client_info.client.clone();
        let query = self.query.clone();
        let count_result = gx
            .modal(LoadingModal::new(
                "Counting records...",
                async move { query.count(&client).await },
            ))
            .await;

        let count = match count_result {
            Some(Ok(count)) => {
                log::debug!("[Export] Total count: {}", count);
                self.total_count.set(Some(count));
                count
            }
            Some(Err(e)) => {
                gx.toast(Toast::error(format!("Failed to count records: {}", e)));
                cx.close();
                return;
            }
            None => {
                // User cancelled
                cx.close();
                return;
            }
        };

        // Fetch all records page by page with progress updates
        let client = self.client_info.client.clone();
        let query = self.query.clone();
        let mut pages = query.page_size(5000).into_async_iter(&client);
        let mut all_records = Vec::new();
        let mut page_num = 0;
        let estimated_pages = (count + 4999) / 5000; // round up

        loop {
            page_num += 1;

            let client_clone = client.clone();
            let fetch_result = gx
                .modal(LoadingModal::new(
                    format!("Fetching page {}/{}...", page_num, estimated_pages),
                    async move {
                        let result = pages.next(&client_clone).await;
                        (result, pages)
                    },
                ))
                .await;

            let Some((page_result, updated_pages)) = fetch_result else {
                // User cancelled
                cx.close();
                return;
            };

            pages = updated_pages;

            match page_result {
                Some(Ok(page)) => {
                    all_records.extend(page.records().to_vec());
                    log::debug!("[Export] Fetched {} records so far", all_records.len());
                }
                Some(Err(e)) => {
                    gx.toast(Toast::error(format!("Failed to fetch records: {}", e)));
                    cx.close();
                    return;
                }
                None => {
                    // No more pages
                    break;
                }
            }
        }

        let records = all_records;

        // Determine columns
        let columns: Vec<String> = if !self.selected_fields.is_empty() {
            self.selected_fields.clone()
        } else if let Some(first) = records.first() {
            first.fields().keys().cloned().collect()
        } else {
            Vec::new()
        };

        // Build preview table
        let rows = convert_records_to_rows(&records, std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)));
        let table_columns: Vec<Column> = columns
            .iter()
            .map(|name| Column::new(name, name).fixed(20))
            .collect();

        let table_state = TableState::new(rows, table_columns).with_selection(SelectionMode::None);

        // Update state
        let record_count = records.len();
        self.records.set(records);
        self.columns.set(columns);
        self.preview_table.set(table_state);

        gx.toast(Toast::info(format!("Loaded {} records", record_count)));
    }

    fn title(&self) -> String {
        let entity_name = self.entity.name();
        format!("Export ({})", entity_name)
    }

    #[keybinds]
    fn keybinds() {
        bind("escape", go_back);
        bind("r", refresh);
        bind("e", export);
    }

    #[handler]
    async fn go_back(&self, gx: &GlobalContext, cx: &AppContext) {
        if let Some(origin_id) = self.origin {
            gx.focus_instance(origin_id);
            cx.close();
        }
    }

    #[handler]
    async fn refresh(&self, gx: &GlobalContext) {
        // TODO: Re-execute query
        gx.toast(Toast::info("Refresh not yet implemented"));
    }

    #[handler]
    async fn export(&self, gx: &GlobalContext) {
        // Generate default filename (without extension)
        let entity_name = self.entity.name();
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let default_filename = format!("{}_{}", entity_name, timestamp);
        
        let start_dir = paths::downloads_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
        let file_types = vec!["csv".to_string(), "xlsx".to_string()];

        let modal = FileBrowserModal::new(&start_dir, file_types)
            .with_filename(default_filename);

        if let Some(result) = gx.modal(modal).await {
            // TODO: Write file based on file_type
            gx.toast(Toast::info(format!(
                "Export to {} as {} not yet implemented",
                result.path.display(),
                result.file_type
            )));
        }
    }

    fn element(&self) -> Element {
        let has_records = self.records.with_ref(|r| !r.is_empty());
        let record_count = self.records.with_ref(|r| r.len());
        let column_count = self.columns.with_ref(|c| c.len());
        let has_origin = self.origin.is_some();

        page! {
            column (padding: (1, 2), gap: 1, height: fill, width: fill) style (bg: background) {
                // Back button
                if has_origin {
                    button (label: "Back", hint: "esc", id: "back") on_activate: go_back()
                }

                // Table area
                if has_records {
                    box_ (id: "table-container", height: fill, width: fill) style (bg: surface) {
                        table (state: self.preview_table, id: "preview-table")
                    }
                } else {
                    box_ (height: fill, width: fill) style (bg: surface) {
                        column (height: fill, width: fill, align: center, justify: center) {
                            text (content: "No records found") style (fg: muted)
                        }
                    }
                }

                // Footer
                row (width: fill, justify: between) {
                    row (gap: 2) {
                        text (content: {format!("{} records", record_count)}) style (fg: muted)
                        text (content: {format!("{} columns", column_count)}) style (fg: muted)
                    }
                    row (gap: 2) {
                        button (label: "Refresh", hint: "r", id: "refresh") on_activate: refresh()
                        if has_records {
                            button (label: "Export", hint: "e", id: "export") on_activate: export()
                        }
                    }
                }
            }
        }
    }
}
