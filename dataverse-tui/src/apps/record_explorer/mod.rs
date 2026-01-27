//! Record Explorer app for viewing Dataverse entity records.

pub mod row;
pub mod service;

pub use row::RecordRow;
pub use service::{convert_records_to_rows, default_columns};

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use dataverse_lib::api::query::odata::{ODataPages, QueryBuilder as ODataQueryBuilder};
use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{Button, Column, SelectionMode, Table, TableState, Text};
use rafter::{EventData, InstanceId};
use tuidom::Element;

use crate::formatting::default_column_width;
use crate::modals::LoadingModal;
use crate::systems::client_management::ActiveClientInfo;
use crate::widgets::Spinner;

use row::EntityData;
use service::fetch_entity_data;

#[app(name = "Record Explorer")]
pub struct RecordExplorer {
    /// Query builder template for refresh.
    #[state(skip)]
    query_template: ODataQueryBuilder,

    /// Current working iterator.
    pages: ODataPages,

    /// Full connection context.
    #[state(skip)]
    client_info: ActiveClientInfo,

    /// Optional origin instance (for back navigation).
    #[state(skip)]
    origin: Option<InstanceId>,

    /// Entity being queried.
    #[state(skip)]
    entity: dataverse_lib::model::Entity,

    /// Selected fields (empty = all fields).
    #[state(skip)]
    selected_fields: Vec<String>,

    /// Entity data (Some after initial load).
    entity_data: Option<EntityData>,

    /// Records table.
    records: TableState<RecordRow>,
    records_loading: Resource<()>,
    total_count: Option<usize>,

    /// Advanced mode - shared with rows for efficient rendering.
    #[state(skip)]
    advanced_mode: Arc<AtomicBool>,
}

impl RecordExplorer {
    pub fn new(
        query: ODataQueryBuilder,
        client_info: ActiveClientInfo,
        origin: Option<InstanceId>,
    ) -> Self {
        let entity = query.entity().clone();
        let selected_fields = query.selected_fields().to_vec();

        let query_template = query.page_size(50);

        // Create initial pages iterator (will be replaced in on_start)
        let pages = query_template.clone().into_async_iter(&client_info.client);

        Self {
            query_template,
            pages: State::new(pages),
            client_info,
            origin,
            entity,
            selected_fields,
            advanced_mode: Arc::new(AtomicBool::new(false)),
            entity_data: State::default(),
            records: State::default(),
            records_loading: Resource::default(),
            total_count: State::default(),
            __handler_registry: Default::default(),
            __derived_cache: Default::default(),
        }
    }
}

#[app_impl]
impl RecordExplorer {
    #[on_start]
    async fn on_start(&self, gx: &GlobalContext) {
        // Resolve entity to logical name for metadata fetch
        let client = self.client_info.client.clone();
        let entity = self.entity.clone();
        let logical_name = match gx
            .modal(LoadingModal::new("Resolving entity", async move {
                client.resolve_entity_logical_name(&entity).await
            }))
            .await
        {
            Some(Ok(name)) => name,
            Some(Err(e)) => {
                gx.toast(Toast::error(format!("Failed to resolve entity: {}", e)));
                return;
            }
            None => return,
        };

        // Fetch entity metadata
        let client = self.client_info.client.clone();
        let logical_name_clone = logical_name.clone();
        let entity_data = match gx
            .modal(LoadingModal::new(
                format!("Loading {}...", logical_name),
                async move { fetch_entity_data(&client, &logical_name_clone).await },
            ))
            .await
        {
            Some(Ok(data)) => data,
            Some(Err(e)) => {
                gx.toast(Toast::error(format!("Failed to load entity: {}", e)));
                return;
            }
            None => return,
        };

        self.entity_data.set(Some(entity_data.clone()));

        // Determine display columns from the selected fields or defaults
        let columns: Vec<String> = if self.selected_fields.is_empty() {
            default_columns(&entity_data.metadata)
        } else {
            self.selected_fields.clone()
        };

        // Get the true record count using $apply=aggregate
        let client = self.client_info.client.clone();
        let query = self.query_template.clone();
        let count_result = gx
            .modal(LoadingModal::new(
                "Counting records...",
                async move { query.count(&client).await },
            ))
            .await;

        match count_result {
            Some(Ok(count)) => {
                log::debug!("[RecordExplorer] Total count: {}", count);
                self.total_count.set(Some(count));
            }
            Some(Err(e)) => {
                log::warn!("[RecordExplorer] Failed to get count: {}", e);
                // Continue without count
            }
            None => {
                // User cancelled
                return;
            }
        }

        // Execute the query
        self.do_load_records(&entity_data, &columns, gx).await;
    }

    fn title(&self) -> String {
        let fallback = self.entity.name().to_string();
        let entity_name = self.entity_data.with_ref(|data| {
            data.as_ref()
                .and_then(|d| d.metadata.display_name.text())
                .map(|s| s.to_string())
                .unwrap_or_else(|| fallback.clone())
        });

        format!("{} ({})", entity_name, self.client_info.environment_name)
    }

    #[keybinds]
    fn keybinds() {
        bind("r", refresh);
        bind("f2", toggle_advanced);
        bind("escape", go_back);
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
        let entity_data = match self.entity_data.get() {
            Some(data) => data,
            None => return,
        };

        let columns: Vec<String> = if self.selected_fields.is_empty() {
            default_columns(&entity_data.metadata)
        } else {
            self.selected_fields.clone()
        };

        // Re-run count query
        let client = self.client_info.client.clone();
        let query = self.query_template.clone();
        let count_result = gx
            .modal(LoadingModal::new(
                "Counting records...",
                async move { query.count(&client).await },
            ))
            .await;

        match count_result {
            Some(Ok(count)) => {
                log::debug!("[RecordExplorer] Total count: {}", count);
                self.total_count.set(Some(count));
            }
            Some(Err(e)) => {
                log::warn!("[RecordExplorer] Failed to get count: {}", e);
                self.total_count.set(None);
            }
            None => {
                // User cancelled
                return;
            }
        }

        self.do_load_records(&entity_data, &columns, gx).await;

        gx.toast(Toast::info("Refreshed"));
    }

    #[handler]
    async fn toggle_advanced(&self, gx: &GlobalContext) {
        let new_mode = !self.advanced_mode.load(Ordering::Relaxed);
        self.advanced_mode.store(new_mode, Ordering::Relaxed);

        if let Some(entity_data) = self.entity_data.get() {
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

        gx.toast(Toast::info(if new_mode {
            "Advanced mode"
        } else {
            "Normal mode"
        }));
    }

    /// Execute the query and populate the table.
    async fn do_load_records(
        &self,
        entity_data: &EntityData,
        columns: &[String],
        _gx: &GlobalContext,
    ) {
        if self.records_loading.is_loading() {
            return;
        }

        self.records_loading.set_loading();

        // Create fresh pages iterator from query template
        let pages = self
            .query_template
            .clone()
            .into_async_iter(&self.client_info.client);
        self.pages.set(pages);

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

        // Fetch the first page
        let mut pages = self.pages.get();
        let page = match pages.next(&self.client_info.client).await {
            Some(Ok(p)) => p,
            Some(Err(e)) => {
                log::error!("[RecordExplorer] Failed to fetch records: {}", e);
                self.records_loading
                    .set_error(format!("Failed to load records: {}", e));
                return;
            }
            None => {
                log::warn!("[RecordExplorer] No records returned");
                self.records_loading.set_ready(());
                return;
            }
        };

        let rows = convert_records_to_rows(page.records(), self.advanced_mode.clone());
        self.pages.set(pages);

        // Update table
        let frozen_col = table_columns.first().map(|c| c.id.clone());
        let mut state = TableState::new(rows, table_columns).with_selection(SelectionMode::None);
        if let Some(col) = &frozen_col {
            state = state.with_frozen(&[col.as_str()]);
        }
        self.records.set(state);

        self.records_loading.set_ready(());
    }

    #[handler]
    async fn on_table_scroll(&self, gx: &GlobalContext, event: &EventData) {
        if event.is_near_bottom(0.8) {
            self.load_more_records(gx).await;
        }
    }

    async fn load_more_records(&self, gx: &GlobalContext) {
        if self.records_loading.is_loading() {
            return;
        }

        self.records_loading.set_progress(ProgressState {
            current: 0,
            total: None,
            message: Some("Loading more records...".to_string()),
        });

        let mut pages = self.pages.get();
        let page = match pages.next(&self.client_info.client).await {
            Some(Ok(p)) => p,
            Some(Err(e)) => {
                log::error!("[RecordExplorer] Failed to fetch more records: {}", e);
                gx.toast(Toast::error("Failed to load more records"));
                self.records_loading.set_ready(());
                return;
            }
            None => {
                // No more pages
                self.records_loading.set_ready(());
                return;
            }
        };

        let new_rows = convert_records_to_rows(page.records(), self.advanced_mode.clone());
        self.pages.set(pages);

        self.records.update(|state| {
            state.extend_rows(new_rows);
        });

        self.records_loading.set_ready(());
    }

    fn element(&self) -> Element {
        let (has_records, loaded_count, column_count) = self
            .records
            .with_ref(|t| (!t.rows.is_empty(), t.rows.len(), t.columns.len()));
        let records_state = self.records_loading.get();
        let total_count = self.total_count.get();
        let has_origin = self.origin.is_some();

        page! {
            column (padding: (1, 2), gap: 1, height: fill, width: fill) style (bg: background) {
                // Back button
                if has_origin {
                    button (label: "Back", hint: "esc", id: "back-button") on_activate: go_back()
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

                // Footer
                row (width: fill, justify: between) {
                    if let Some(total) = total_count {
                        text (content: {format!("{}/{}", loaded_count, total)}) style (fg: muted)
                    } else {
                        text (content: {format!("{} records", loaded_count)}) style (fg: muted)
                    }

                    match records_state {
                        ResourceState::Progress(_) => {
                            spinner (id: "pagination-spinner")
                        }
                        _ => {}
                    }

                    text (content: {format!("{} columns", column_count)}) style (fg: muted)
                }
            }
        }
    }
}
