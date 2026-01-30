//! Import app for importing data from CSV/Excel files into Dataverse.

mod io;
mod modals;

use std::path::PathBuf;

use dataverse_lib::model::Entity;
use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{Button, Column, SelectionMode, Table, TableState, Text};

use crate::apps::record_explorer::RecordRow;
use crate::file_io::{ParsedFile, list_sheets, read_csv, read_excel};
use crate::formatting::FormattedValue;
use crate::modals::{FileBrowserModal, LoadingModal, SheetSelectorModal};
use crate::paths;
use crate::systems::client_management::ActiveClientInfo;

use self::modals::ImportSettingsModal;

/// Import app: load file, configure, send to queue.
#[app(name = "Import")]
pub struct Import {
    /// Client connection info.
    #[state(skip)]
    client_info: ActiveClientInfo,

    /// Parsed file data.
    parsed_file: Option<ParsedFile>,

    /// Preview table state.
    preview_table: TableState<RecordRow>,
}

impl Import {
    pub fn with_client(client_info: ActiveClientInfo) -> Self {
        Self {
            client_info,
            parsed_file: State::default(),
            preview_table: State::default(),
            __handler_registry: Default::default(),
            __derived_cache: Default::default(),
        }
    }
}

#[app_impl]
impl Import {
    fn title(&self) -> String {
        "Import".to_string()
    }

    #[keybinds]
    fn keybinds() {
        bind("escape", close);
        bind("o", open_file);
        bind("i", import);
    }

    #[handler]
    async fn close(&self, _gx: &GlobalContext, cx: &AppContext) {
        cx.close();
    }

    #[handler]
    async fn open_file(&self, gx: &GlobalContext) {
        // Show file browser modal for CSV and Excel files
        let start_dir = paths::downloads_dir().unwrap_or_else(|| PathBuf::from("."));
        let result = gx
            .modal(
                FileBrowserModal::browse(&start_dir, vec!["csv".to_string(), "xlsx".to_string()])
                    .require_existing(),
            )
            .await;

        let Some(file_result) = result else {
            return; // User cancelled
        };

        // If it's an Excel file, show sheet selector modal first
        let selected_sheet = if file_result.file_type == "xlsx" {
            match list_sheets(&file_result.path) {
                Ok(sheets) => {
                    let selected = gx.modal(SheetSelectorModal::with_sheets(sheets)).await;
                    if selected.is_none() {
                        // User cancelled sheet selection
                        return;
                    }
                    selected
                }
                Err(e) => {
                    gx.toast(Toast::error(format!("Failed to read Excel file: {}", e)));
                    return;
                }
            }
        } else {
            None
        };

        // Parse the file immediately
        let path = file_result.path.clone();
        let ftype = file_result.file_type.clone();
        let parsed_result = gx
            .modal(LoadingModal::new("Parsing file...", async move {
                if ftype == "xlsx" {
                    read_excel(&path, selected_sheet.as_deref())
                } else {
                    read_csv(&path)
                }
            }))
            .await;

        let parsed = match parsed_result {
            Some(Ok(p)) => p,
            Some(Err(e)) => {
                gx.toast(Toast::error(format!("Failed to parse file: {}", e)));
                return;
            }
            None => return, // User cancelled
        };

        // Build preview table
        let columns: Vec<Column> = parsed
            .columns
            .iter()
            .map(|name| Column::new(name.clone(), name.clone()))
            .collect();

        let advanced_mode = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let rows: Vec<RecordRow> = parsed
            .rows
            .iter()
            .enumerate()
            .map(|(idx, row)| {
                let mut record_row = RecordRow::new(idx.to_string(), advanced_mode.clone());
                for (col_idx, value) in row.values.iter().enumerate() {
                    let column_name = parsed.columns.get(col_idx).cloned().unwrap_or_default();
                    let val_str = value.clone().unwrap_or_default();
                    let formatted = FormattedValue::new(val_str.clone(), val_str);
                    record_row.set_cell(column_name, formatted);
                }
                record_row
            })
            .collect();

        let table_state = TableState::new(rows, columns).with_selection(SelectionMode::None);

        self.preview_table.set(table_state);
        self.parsed_file.set(Some(parsed));

        gx.toast(Toast::info("File loaded successfully"));
    }

    #[handler]
    async fn import(&self, gx: &GlobalContext) {
        // Get parsed file
        let parsed = self.parsed_file.with_ref(|p| p.clone());

        let Some(parsed) = parsed else {
            gx.toast(Toast::error("No file loaded"));
            return;
        };

        // For Excel: use sheet name as suggested entity (sheet name IS the entity name)
        // For CSV: no suggestion
        let suggested_entity = parsed
            .sheet_name
            .as_ref()
            .map(|name| name.trim().to_string());

        // Fetch entity list from Dataverse
        let client = self.client_info.client.clone();
        let entities_result = gx
            .modal(LoadingModal::new("Fetching entities...", async move {
                client.metadata().all_entities().await
            }))
            .await;

        let entities = match entities_result {
            Some(Ok(e)) => e,
            Some(Err(e)) => {
                gx.toast(Toast::error(format!("Failed to fetch entities: {}", e)));
                return;
            }
            None => return,
        };

        // Build entity options: (entity_set, display_label)
        let entity_options: Vec<(String, String)> = entities
            .iter()
            .map(|e| {
                let display = e.display_name.text().unwrap_or(&e.logical_name).to_string();
                (e.entity_set_name.clone(), display)
            })
            .collect();

        // Show settings modal (it will fetch primary key internally)
        let settings = gx
            .modal(ImportSettingsModal::with_config(
                self.client_info.client.clone(),
                suggested_entity,
                entity_options,
                parsed.columns.clone(),
                parsed.rows.clone(),
            ))
            .await;

        let Some(settings) = settings else {
            return; // User cancelled
        };

        // 1. Fetch attributes for metadata validation
        let client = self.client_info.client.clone();
        let entity_set = settings.entity_set.clone();
        let attributes_result = gx
            .modal(LoadingModal::new(
                "Fetching attribute metadata...",
                async move { client.metadata().attributes(Entity::set(&entity_set)).await },
            ))
            .await;

        let attributes = match attributes_result {
            Some(Ok(attrs)) => attrs,
            Some(Err(e)) => {
                gx.toast(Toast::error(format!("Failed to fetch attributes: {}", e)));
                return;
            }
            None => return,
        };

        // Build attributes map
        let attributes_map: std::collections::HashMap<
            String,
            dataverse_lib::model::metadata::AttributeMetadata,
        > = attributes
            .into_iter()
            .map(|a| (a.logical_name.clone(), a))
            .collect();

        // 2. Get primary key from entity metadata
        let client = self.client_info.client.clone();
        let entity_set = settings.entity_set.clone();
        let primary_key_result = gx
            .modal(LoadingModal::new("Fetching primary key...", async move {
                let metadata = client.metadata().entity(Entity::set(&entity_set)).await?;
                Ok::<_, dataverse_lib::error::Error>(metadata.primary_id_attribute.to_string())
            }))
            .await;

        let primary_key = match primary_key_result {
            Some(Ok(pk)) => pk,
            Some(Err(e)) => {
                gx.toast(Toast::error(format!("Failed to fetch primary key: {}", e)));
                return;
            }
            None => return,
        };

        // 3. Parse headers to identify lookup columns
        let columns = io::parse_headers(&parsed.columns);

        // 4. Convert rows to operations
        let rows = parsed.rows.clone();
        let entity = Entity::set(settings.entity_set.clone());
        let (operations, errors) = gx
            .modal(LoadingModal::new(
                "Converting rows to operations...",
                async move {
                    io::rows_to_operations(&rows, &columns, &attributes_map, entity, &primary_key)
                },
            ))
            .await
            .unwrap_or_else(|| (vec![], vec![]));

        // 5. Handle errors - STOP if any exist
        if !errors.is_empty() {
            let error_messages: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
            gx.modal(crate::modals::ErrorModal::with_errors(
                "Import Conversion Failed",
                error_messages,
            ))
            .await;
            return;
        }

        // 6. Split into batches and send to Queue
        use crate::apps::queue::Queue;
        use crate::apps::queue::api::{AddItems, NewItem};
        use crate::apps::queue::types::QueuePayload;
        use dataverse_lib::api::Batch;

        let batch_size = settings.batch_size;
        let total_ops = operations.len();
        let total_batches = total_ops.div_ceil(batch_size);

        for (batch_idx, chunk) in operations.chunks(batch_size).enumerate() {
            let mut batch = Batch::new();
            for op in chunk {
                batch = batch.add(op.clone());
            }

            let item = NewItem {
                priority: 0,
                payload: QueuePayload::Batch(batch),
                env_id: self.client_info.env_id,
                account_id: self.client_info.account_id,
                source: "import".to_string(),
                description: format!(
                    "Import batch {}/{} to {}",
                    batch_idx + 1,
                    total_batches,
                    settings.entity_set
                ),
            };

            // Send to queue
            match gx
                .request::<Queue, AddItems>(AddItems { items: vec![item] })
                .await
            {
                Ok(_) => {}
                Err(e) => {
                    gx.toast(Toast::error(format!("Queue request failed: {}", e)));
                    return;
                }
            }
        }

        gx.toast(Toast::success(format!(
            "Sent {} operations in {} batches to queue",
            total_ops, total_batches
        )));
    }

    fn element(&self) -> Element {
        let has_file = self.parsed_file.with_ref(|p| p.is_some());

        page! {
            column (padding: (1, 2), gap: 1, height: fill, width: fill) style (bg: background) {
                // Title
                text (content: "Import") style (fg: interact)

                // Content
                if has_file {
                    box_ (height: fill, width: fill) style (bg: surface) {
                        table (state: self.preview_table, id: "preview-table")
                    }
                } else {
                    box_ (height: fill, width: fill) style (bg: surface) {
                        column (height: fill, width: fill, align: center, justify: center, gap: 1) {
                            text (content: "No file loaded") style (fg: muted)
                            text (content: "Press 'o' to open a file") style (fg: primary)
                        }
                    }
                }

                // Footer
                row (width: fill, justify: between) {
                    button (label: "Close", hint: "esc", id: "close") on_activate: close()
                    row (gap: 1) {
                        button (label: "Open", hint: "o", id: "open") on_activate: open_file()
                        if has_file {
                            button (label: "Import", hint: "i", id: "import") on_activate: import()
                        }
                    }
                }
            }
        }
    }
}
