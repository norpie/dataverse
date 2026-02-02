//! Export app for exporting Dataverse query results to CSV or Excel.

mod io;

use std::collections::HashMap;

use dataverse_lib::error::Error as DataverseError;
use dataverse_lib::model::{Entity, Record};
use rafter::InstanceId;
use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{Button, Column, SelectionMode, Table, TableState, Text};

use crate::apps::record_explorer::{RecordRow, convert_records_to_rows};
use crate::file_io::{FileIoError, write_csv, write_excel};
use crate::modals::{FileBrowserModal, LoadingModal};
use crate::paths;
use crate::systems::client_management::ActiveClientInfo;

use io::{LookupColumns, records_to_rows, transform_headers};

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
    pub fn with_query(
        query: dataverse_lib::api::query::odata::QueryBuilder,
        client_info: ActiveClientInfo,
        origin: Option<InstanceId>,
    ) -> Self {
        let entity = query.entity().clone();
        let selected_fields = query.selected_fields().to_vec();

        Self::new(
            query,
            client_info,
            origin,
            entity,
            selected_fields,
            Vec::new(),
            Vec::new(),
            TableState::default(),
            None,
        )
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
            .modal(LoadingModal::run_with_default(
                "Counting records...",
                || Err(DataverseError::Cancelled),
                async move { query.count(&client).await },
            ))
            .await;

        let count = match count_result {
            Ok(count) => {
                log::debug!("[Export] Total count: {}", count);
                self.total_count.set(Some(count));
                count
            }
            Err(e) if e.is_cancelled() => {
                cx.close();
                return;
            }
            Err(e) => {
                gx.toast(Toast::error(format!("Failed to count records: {}", e)));
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
        let estimated_pages = count.div_ceil(5000); // round up

        loop {
            page_num += 1;

            let client_clone = client.clone();
            let pages_for_default = pages.clone();
            let (page_result, updated_pages) = gx
                .modal(LoadingModal::run_with_default(
                    format!("Fetching page {}/{}...", page_num, estimated_pages),
                    move || (None, pages_for_default.clone()),
                    async move {
                        let result = pages.next(&client_clone).await;
                        (result, pages)
                    },
                ))
                .await;

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
                    // No more pages (or shutdown)
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
        let rows = convert_records_to_rows(
            &records,
            std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        );
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
        // Get the true record count using $apply=aggregate
        let client = self.client_info.client.clone();
        let query = self.query.clone();
        let count_result = gx
            .modal(LoadingModal::run_with_default(
                "Counting records...",
                || Err(DataverseError::Cancelled),
                async move { query.count(&client).await },
            ))
            .await;

        let count = match count_result {
            Ok(count) => {
                self.total_count.set(Some(count));
                count
            }
            Err(e) if e.is_cancelled() => return,
            Err(e) => {
                gx.toast(Toast::error(format!("Failed to count records: {}", e)));
                return;
            }
        };

        // Fetch all records page by page with progress updates
        let client = self.client_info.client.clone();
        let query = self.query.clone();
        let mut pages = query.page_size(5000).into_async_iter(&client);
        let mut all_records = Vec::new();
        let mut page_num = 0;
        let estimated_pages = count.div_ceil(5000);

        loop {
            page_num += 1;

            let client_clone = client.clone();
            let pages_for_default = pages.clone();
            let (page_result, updated_pages) = gx
                .modal(LoadingModal::run_with_default(
                    format!("Fetching page {}/{}...", page_num, estimated_pages),
                    move || (None, pages_for_default.clone()),
                    async move {
                        let result = pages.next(&client_clone).await;
                        (result, pages)
                    },
                ))
                .await;

            pages = updated_pages;

            match page_result {
                Some(Ok(page)) => {
                    all_records.extend(page.records().to_vec());
                }
                Some(Err(e)) => {
                    gx.toast(Toast::error(format!("Failed to fetch records: {}", e)));
                    return;
                }
                None => break,
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
        let rows = convert_records_to_rows(
            &records,
            std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        );
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

        gx.toast(Toast::info(format!("Refreshed {} records", record_count)));
    }

    #[handler]
    async fn export(&self, gx: &GlobalContext) {
        // Generate default filename (without extension)
        let entity_name = self.entity.name().to_string();
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let default_filename = format!("{}_{}", entity_name, timestamp);

        let start_dir = paths::downloads_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
        let file_types = vec!["csv".to_string(), "xlsx".to_string()];

        let modal =
            FileBrowserModal::browse(&start_dir, file_types).with_filename(default_filename);

        let Some(result) = gx.modal(modal).await else {
            return;
        };

        let records = self.records.with_ref(|r| r.clone());
        let columns = self.columns.with_ref(|c| c.clone());

        // Fetch entity metadata to identify lookup columns
        let client = self.client_info.client.clone();
        let entity = self.entity.clone();
        let cols = columns.clone();

        let metadata_result = gx
            .modal(LoadingModal::run_with_default(
                "Fetching metadata...",
                || Err(DataverseError::Cancelled),
                async move { client.metadata().attributes(entity).await },
            ))
            .await;

        let attributes = match metadata_result {
            Ok(attrs) => attrs,
            Err(e) if e.is_cancelled() => return,
            Err(e) => {
                gx.toast(Toast::error(format!("Failed to fetch metadata: {}", e)));
                return;
            }
        };

        // Build lookup columns map: column_name -> target entity set name
        // Only include columns that are in our export and are lookups
        let mut lookup_columns: LookupColumns = HashMap::new();
        let mut targets_to_resolve: Vec<(String, String)> = Vec::new(); // (column, target_logical_name)

        for attr in &attributes {
            if attr.is_lookup() && cols.contains(&attr.logical_name) {
                // Use first target for polymorphic lookups (Customer, Owner)
                if let Some(target) = attr.targets.first() {
                    targets_to_resolve.push((attr.logical_name.clone(), target.clone()));
                }
            }
        }

        // Resolve target entity set names
        let total = targets_to_resolve.len();
        for (i, (column, target_logical_name)) in targets_to_resolve.into_iter().enumerate() {
            let client = self.client_info.client.clone();
            let target = target_logical_name.clone();

            let resolve_result = gx
                .modal(LoadingModal::run_with_default(
                    format!("Resolving lookup {}/{}...", i + 1, total),
                    || Err(DataverseError::Cancelled),
                    async move { client.resolve_entity_set_name(&target).await },
                ))
                .await;

            match resolve_result {
                Ok(entity_set) => {
                    lookup_columns.insert(column, entity_set);
                }
                Err(e) if e.is_cancelled() => return,
                Err(e) => {
                    gx.toast(Toast::error(format!(
                        "Failed to resolve entity set for {}: {}",
                        target_logical_name, e
                    )));
                    return;
                }
            }
        }

        // Transform records to string rows
        let rows = records_to_rows(&records, &columns, &lookup_columns);
        let headers = transform_headers(&columns, &lookup_columns);
        let path = result.path.clone();
        let file_type = result.file_type.clone();
        let sheet_name = entity_name.clone();
        let record_count = records.len();

        // Write file in blocking task with loading modal
        let write_result = gx
            .modal(LoadingModal::run_with_default(
                "Exporting...",
                || Err(FileIoError::Cancelled),
                async move {
                    tokio::task::spawn_blocking(move || match file_type.as_str() {
                        "csv" => write_csv(&path, &headers, &rows),
                        "xlsx" => write_excel(&path, &headers, &rows, &sheet_name),
                        _ => Err(FileIoError::Io(std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            format!("Unsupported file type: {}", file_type),
                        ))),
                    })
                    .await
                    .map_err(|e| {
                        FileIoError::Io(std::io::Error::other(format!("Task join error: {}", e)))
                    })?
                },
            ))
            .await;

        match write_result {
            Ok(()) => {
                gx.toast(Toast::success(format!(
                    "Exported {} records to {}",
                    record_count,
                    result.path.display()
                )));
            }
            Err(e) if e.is_cancelled() => {}
            Err(e) => {
                gx.toast(Toast::error(format!("Export failed: {}", e)));
            }
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
