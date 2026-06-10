//! VAF deadline import app.

mod diff;
mod excel;
mod fetch;
mod operations;
mod scope;
mod transform;
mod types;

use std::path::PathBuf;

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{Button, Column, SelectionMode, Table, TableState, Text};
use tuidom::Element;

use crate::apps::queue::Queue;
use crate::apps::queue::api::AddItems;
use crate::file_io::list_sheets;
use crate::modals::odata_fetch::ODataFetchModal;
use crate::modals::{
    ConfirmModal, ErrorModal, FileBrowserModal, LoadingModal, LoadingUpdater, SheetSelectorModal,
};
use crate::paths;
use crate::systems::client_management::{ActiveClientInfo, ClientManagement, GetActiveClient};

use self::fetch::{build_fetch_tasks, build_import_context, fetch_metadata};
use self::operations::build_queue_items;
use self::transform::transform_workbook;
use self::types::{DeadlineMode, DeadlineRecord, DeadlineTableRow, ImportData, LookupCache};

#[app(name = "VAF - Deadline Import", singleton, on_blur = Close, default)]
pub struct DeadlineImport {
    client_info: Option<ActiveClientInfo>,
    lookup_cache: Option<LookupCache>,
    import_data: Option<ImportData>,
    table: TableState<DeadlineTableRow>,
    load_error: Option<String>,
}

impl DeadlineImport {
    pub fn with_client(client_info: ActiveClientInfo) -> Self {
        Self::new(Some(client_info), None, None, TableState::default(), None)
    }
}

#[app_impl]
impl DeadlineImport {
    #[on_start]
    async fn on_start(&self, gx: &GlobalContext) {
        if self.ensure_client(gx).await {
            self.open_file(gx).await;
        }
    }

    fn title(&self) -> String {
        let env_name = self
            .client_info
            .with_ref(|info| info.as_ref().map(|info| info.environment_name.clone()))
            .unwrap_or_else(|| "No active environment".to_string());
        format!("VAF - Deadline Import — {}", env_name)
    }

    #[keybinds]
    fn keybinds() {
        bind("escape", close);
        bind("o", open_file);
        bind("q", queue);
    }

    #[handler]
    async fn close(&self, cx: &AppContext) {
        cx.close();
    }

    #[handler]
    async fn open_file(&self, gx: &GlobalContext) {
        if !self.ensure_client(gx).await {
            return;
        }
        let start_dir = paths::downloads_dir().unwrap_or_else(|| PathBuf::from("."));
        let Some(file_result) = gx
            .modal(
                FileBrowserModal::browse(&start_dir, vec!["xlsx".to_string()]).require_existing(),
            )
            .await
        else {
            return;
        };

        let sheets = match list_sheets(&file_result.path) {
            Ok(sheets) => sheets,
            Err(e) => {
                gx.modal(ErrorModal::with_message(
                    "Excel Error",
                    format!("Failed to read workbook: {e}"),
                ))
                .await;
                return;
            }
        };

        let Some(sheet_name) = gx.modal(SheetSelectorModal::with_sheets(sheets)).await else {
            return;
        };

        self.process_import(gx, file_result.path, sheet_name).await;
    }

    #[handler]
    async fn queue(&self, gx: &GlobalContext) {
        let Some(import_data) = self.import_data.get() else {
            gx.toast(Toast::error("No import loaded"));
            return;
        };
        let Some(cache) = self.lookup_cache.get() else {
            gx.toast(Toast::error("Lookup cache is not available"));
            return;
        };

        let create_count = import_data
            .records
            .iter()
            .filter(|record| record.mode == DeadlineMode::Create && record.is_actionable())
            .count();
        let update_count = import_data
            .records
            .iter()
            .filter(|record| record.mode == DeadlineMode::Update && record.is_actionable())
            .count();
        let actionable = create_count + update_count;

        if actionable == 0 {
            gx.toast(Toast::warning("No actionable rows to queue"));
            return;
        }

        let confirmed = gx
            .modal(
                ConfirmModal::with_message(format!(
                    "Queue {} deadline operation(s)?\n\nCreates: {}\nUpdates: {}",
                    actionable, create_count, update_count
                ))
                .title("Queue deadline import"),
            )
            .await;
        if !confirmed {
            return;
        }

        let Some(client_info) = self.client_info.get() else {
            gx.toast(Toast::error("No active client"));
            return;
        };
        let items = build_queue_items(&import_data.records, &cache, &client_info);
        if items.is_empty() {
            gx.toast(Toast::warning("No queue items were generated"));
            return;
        }

        match gx.request::<Queue, AddItems>(AddItems { items }).await {
            Ok(response) => gx.toast(Toast::success(format!(
                "Queued {} deadline import item(s)",
                response.ids.len()
            ))),
            Err(e) => gx.toast(Toast::error(format!("Failed to queue import: {:?}", e))),
        }
    }

    async fn ensure_client(&self, gx: &GlobalContext) -> bool {
        if self.client_info.get().is_some() {
            return true;
        }
        match gx
            .request_system::<ClientManagement, GetActiveClient>(GetActiveClient)
            .await
        {
            Ok(Ok(info)) => {
                self.client_info.set(Some(info));
                true
            }
            Ok(Err(e)) => {
                gx.toast(Toast::error(format!("Client error: {}", e)));
                false
            }
            Err(e) => {
                gx.toast(Toast::error(format!(
                    "No active client. Please configure a connection first. ({:?})",
                    e
                )));
                false
            }
        }
    }

    async fn process_import(&self, gx: &GlobalContext, file_path: PathBuf, sheet_name: String) {
        self.load_error.set(None);
        self.import_data.set(None);
        self.table.set(TableState::default());
        self.lookup_cache.set(None);

        let Some(client_info) = self.client_info.get() else {
            gx.toast(Toast::error("No active client"));
            return;
        };
        let client = client_info.client.clone();
        let metadata = match gx
            .modal(LoadingModal::run_with_default(
                "Loading NRQ metadata...",
                || Err("Metadata loading cancelled".to_string()),
                async move { fetch_metadata(&client).await },
            ))
            .await
        {
            Ok(metadata) => metadata,
            Err(e) => {
                self.load_error.set(Some(e.clone()));
                gx.modal(ErrorModal::with_message("Metadata Error", e))
                    .await;
                return;
            }
        };

        let (tasks, index) = build_fetch_tasks(client_info.client.clone());
        let fetch_results = match gx.modal(ODataFetchModal::create(tasks)).await {
            Ok(results) => results,
            Err(e) => {
                let message = format!("Failed to load NRQ data: {e}");
                self.load_error.set(Some(message.clone()));
                gx.modal(ErrorModal::with_message("Fetch Error", message))
                    .await;
                return;
            }
        };

        let process_result = gx
            .modal(LoadingModal::run_with_default_updates(
                "Processing deadline import...",
                || Err("Processing cancelled".to_string()),
                |updater: LoadingUpdater| {
                    let file_path = file_path.clone();
                    let sheet_name = sheet_name.clone();
                    async move {
                        updater.update("Reading Excel sheet...");
                        let workbook = excel::read_deadline_sheet(&file_path, &sheet_name)?;
                        updater.update("Building lookup cache...");
                        let (cache, existing_deadlines) =
                            build_import_context(fetch_results, index, metadata)?;
                        updater.update("Transforming rows...");
                        let context = types::ImportContext {
                            cache: cache.clone(),
                            existing_deadlines,
                        };
                        let mut data = transform_workbook(workbook, context, file_path, sheet_name);
                        updater.update("Building diffs...");
                        diff::apply_diffs(&mut data.records);
                        Ok::<_, String>((data, cache))
                    }
                },
            ))
            .await;

        let (data, cache) = match process_result {
            Ok(result) => result,
            Err(e) => {
                self.load_error.set(Some(e.clone()));
                gx.modal(ErrorModal::with_message("Processing Error", e))
                    .await;
                return;
            }
        };

        self.lookup_cache.set(Some(cache));
        self.table.set(build_table(&data.records));
        self.import_data.set(Some(data));
        gx.toast(Toast::success("Deadline import loaded"));
    }

    fn element(&self) -> Element {
        let env_name = self
            .client_info
            .with_ref(|info| info.as_ref().map(|info| info.environment_name.clone()))
            .unwrap_or_else(|| "No active environment".to_string());
        let load_error = self.load_error.get();
        let import_data = self.import_data.get();
        let selected_detail = import_data
            .as_ref()
            .and_then(|data| selected_record(&self.table.get(), &data.records).map(detail_text));

        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: background) {
                text (content: "VAF - Deadline Import") style (bold, fg: interact)
                row (width: fill, justify: between) {
                    text (content: {format!("Environment: {}", env_name)}) style (fg: primary)
                    row (gap: 1) {
                        button (label: "Open", hint: "o", id: "deadline-import-open") on_activate: open_file()
                        button (label: "Queue", hint: "q", id: "deadline-import-queue") on_activate: queue()
                    }
                }

                if let Some(error) = load_error {
                    text (content: {error}) style (fg: error)
                }

                if let Some(data) = import_data {
                    row (width: fill, justify: between) {
                        text (content: {format!("File: {}", file_name_text(&data.file_path))}) style (fg: muted)
                        text (content: {format!("Sheet: {}", data.sheet_name)}) style (fg: muted)
                        text (content: {format!("{} · {} warning note(s)", summary_text(&data.records), data.warnings.len())}) style (fg: primary)
                    }
                    row (width: fill, height: fill, gap: 1) {
                        box_ (id: "deadline-import-table-container", width: {tuidom::Size::Flex(3)}, height: fill) style (bg: surface) {
                            table (state: self.table, id: "deadline-import-table", width: fill, height: fill)
                        }
                        box_ (id: "deadline-import-detail", width: {tuidom::Size::Flex(2)}, height: fill) style (bg: surface) {
                            column (padding: (1, 2), gap: 1, width: fill, height: fill) {
                                text (content: "Selected deadline") style (bold, fg: interact)
                                if let Some(detail) = selected_detail {
                                    text (content: {detail}) style (fg: primary)
                                } else {
                                    text (content: "Focus a row to inspect it.") style (fg: muted)
                                }
                            }
                        }
                    }
                } else {
                    box_ (width: fill, height: fill) style (bg: surface) {
                        column (padding: (1, 2), gap: 1) {
                            text (content: "No deadline import loaded.") style (fg: muted)
                            text (content: "Press o to select an Excel workbook.") style (fg: muted)
                        }
                    }
                }

                row (width: fill, justify: between) {
                    text (content: "esc close") style (fg: muted)
                    text (content: "o open · q queue") style (fg: muted)
                }
            }
        }
    }
}

fn file_name_text(path: &std::path::Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_default()
}

fn build_table(records: &[DeadlineRecord]) -> TableState<DeadlineTableRow> {
    let columns = vec![
        Column::new("row", "Row").fixed(8),
        Column::new("mode", "Mode").fixed(12),
        Column::new("id", "ID").fixed(36),
        Column::new("name", "Deadline").flex(1),
        Column::new("warnings", "Warn").fixed(6),
    ];
    let rows = records
        .iter()
        .enumerate()
        .map(|(idx, record)| DeadlineTableRow {
            key: idx,
            row: record.source_row,
            mode: record.action_label().to_string(),
            id: record.id.to_string(),
            name: record.name().to_string(),
            warnings: record.warnings.len(),
        })
        .collect();
    TableState::new(rows, columns)
        .with_selection(SelectionMode::Single)
        .with_frozen(&["row", "mode"])
}

fn selected_record<'a>(
    table: &TableState<DeadlineTableRow>,
    records: &'a [DeadlineRecord],
) -> Option<&'a DeadlineRecord> {
    let key = table.focused_key.or(table.last_activated).unwrap_or(0);
    records.get(key)
}

fn summary_text(records: &[DeadlineRecord]) -> String {
    let creates = records
        .iter()
        .filter(|record| record.mode == DeadlineMode::Create)
        .count();
    let updates = records
        .iter()
        .filter(|record| record.mode == DeadlineMode::Update)
        .count();
    let unchanged = records
        .iter()
        .filter(|record| record.mode == DeadlineMode::Unchanged)
        .count();
    let errors = records
        .iter()
        .filter(|record| matches!(record.mode, DeadlineMode::Error(_)))
        .count();
    let warnings = records
        .iter()
        .filter(|record| !record.warnings.is_empty())
        .count();
    format!(
        "{} rows · {} create · {} update · {} unchanged · {} error · {} warning",
        records.len(),
        creates,
        updates,
        unchanged,
        errors,
        warnings
    )
}

fn detail_text(record: &DeadlineRecord) -> String {
    let mut lines = Vec::new();
    lines.push(format!("Row: {}", record.source_row));
    lines.push(format!("Mode: {}", record.action_label()));
    lines.push(format!("ID: {}", record.id));
    if record.id_from_excel.is_none() {
        lines.push("ID source: generated".to_string());
    } else {
        lines.push("ID source: Excel".to_string());
    }
    lines.push(format!("Name: {}", record.name()));
    if let Some(date) = record.fields.deadline_date {
        lines.push(format!("Deadline date: {}", date));
    }
    if let Some(existing) = &record.existing {
        lines.push(format!("Existing ID: {}", existing.id));
    }
    if !record.fields.lookups.is_empty() {
        lines.push("Lookups:".to_string());
        for (field, lookup) in &record.fields.lookups {
            lines.push(format!(
                "- {}: {} ({})",
                field, lookup.label, lookup.target_entity
            ));
        }
    }
    lines.push(format!(
        "Fields changed: {}",
        diff::field_change_count(record)
    ));
    if let Some(existing) = &record.existing {
        let assoc = diff::diff_associations(record, &existing.associations);
        lines.push(format!(
            "Support: +{} -{}",
            assoc.support_to_add.len(),
            assoc.support_to_remove.len()
        ));
        for removed in &assoc.support_to_remove {
            lines.push(format!(
                "  remove support: {} ({})",
                removed.name, removed.related_id
            ));
        }
        lines.push(format!(
            "Category: +{} -{}",
            assoc.category_to_add.len(),
            assoc.category_to_remove.len()
        ));
        lines.push(format!(
            "Subcategory: +{} -{}",
            assoc.subcategory_to_add.len(),
            assoc.subcategory_to_remove.len()
        ));
        lines.push(format!(
            "Flemish share: +{} -{}",
            assoc.flemishshare_to_add.len(),
            assoc.flemishshare_to_remove.len()
        ));
    } else {
        lines.push(format!("Support: {}", record.associations.support.len()));
        lines.push(format!("Category: {}", record.associations.category.len()));
        lines.push(format!(
            "Subcategory: {}",
            record.associations.subcategory.len()
        ));
        lines.push(format!(
            "Flemish share: {}",
            record.associations.flemishshare.len()
        ));
    }
    if let Some(notes) = &record.notes {
        lines.push(format!("OPM: {}", notes));
    }
    if !record.warnings.is_empty() {
        lines.push(String::new());
        lines.push("Warnings:".to_string());
        for warning in &record.warnings {
            lines.push(format!("- {}", warning));
        }
    }
    lines.join("\n")
}
