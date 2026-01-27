//! Record Explorer app for viewing Dataverse entity records.

mod row;
mod service;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use dataverse_lib::api::query::odata::ODataPages;
use dataverse_lib::api::query::odata::QueryBuilder as ODataQueryBuilder;
use rafter::EventData;
use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{Column, SelectionMode, Table, TableState, Text};
use tuidom::Element;

use crate::formatting::default_column_width;
use crate::systems::client_management::ActiveClientInfo;
use crate::widgets::{Spinner, loading_overlay};

use row::{EntityData, RecordRow};
use service::{convert_records_to_rows, default_columns, fetch_entity_data};

#[app(name = "Record Explorer")]
pub struct RecordExplorer {
    /// Fresh iterator template for refresh.
    #[state(skip)]
    pages_template: ODataPages,

    /// Current working iterator.
    pages: ODataPages,

    /// Full connection context.
    #[state(skip)]
    client_info: ActiveClientInfo,

    /// Entity being queried.
    #[state(skip)]
    entity: dataverse_lib::model::Entity,

    /// Selected fields (empty = all fields).
    #[state(skip)]
    selected_fields: Vec<String>,

    /// Loading overlay message (None = no overlay).
    loading_message: Option<String>,

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
    pub fn new(query: ODataQueryBuilder, client_info: ActiveClientInfo) -> Self {
        let entity = query.entity().clone();
        let selected_fields = query.selected_fields().to_vec();
        
        let pages_template = query
            .include_count()
            .page_size(50)
            .into_async_iter(&client_info.client);
        
        Self {
            pages_template: pages_template.clone(),
            pages: State::new(pages_template),
            client_info,
            entity,
            selected_fields,
            advanced_mode: Arc::new(AtomicBool::new(false)),
            loading_message: State::default(),
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
        self.loading_message
            .set(Some("Loading entity...".to_string()));

        // Resolve entity to logical name for metadata fetch
        let logical_name = match self.client_info.client.resolve_entity_logical_name(&self.entity).await {
            Ok(name) => name,
            Err(e) => {
                gx.toast(Toast::error(format!("Failed to resolve entity: {}", e)));
                self.loading_message.set(None);
                return;
            }
        };

        self.loading_message
            .set(Some(format!("Loading {}...", logical_name)));

        // Fetch entity metadata
        let entity_data = match fetch_entity_data(&self.client_info.client, &logical_name).await {
            Ok(data) => data,
            Err(e) => {
                gx.toast(Toast::error(format!("Failed to load entity: {}", e)));
                self.loading_message.set(None);
                return;
            }
        };

        self.entity_data.set(Some(entity_data.clone()));

        // Determine display columns from the selected fields or defaults
        let columns: Vec<String> = if self.selected_fields.is_empty() {
            default_columns(&entity_data.metadata)
        } else {
            self.selected_fields.clone()
        };

        // Execute the query
        self.do_load_records(&entity_data, &columns, gx).await;

        self.loading_message.set(None);
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
    }

    #[handler]
    async fn refresh(&self, gx: &GlobalContext) {
        let display_name = self.entity_data.with_ref(|data| {
            data.as_ref()
                .and_then(|d| d.metadata.display_name.text())
                .map(|s| s.to_string())
                .unwrap_or_else(|| self.entity.name().to_string())
        });

        self.loading_message
            .set(Some(format!("Refreshing {}...", display_name)));

        let entity_data = match self.entity_data.get() {
            Some(data) => data,
            None => {
                self.loading_message.set(None);
                return;
            }
        };

        let columns: Vec<String> = if self.selected_fields.is_empty() {
            default_columns(&entity_data.metadata)
        } else {
            self.selected_fields.clone()
        };

        // Clone the fresh template for refresh
        self.pages.set(self.pages_template.clone());

        self.do_load_records(&entity_data, &columns, gx).await;

        self.loading_message.set(None);
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

        if let Some(count) = page.total_count() {
            log::debug!("[RecordExplorer] Total count: {}", count);
            self.total_count.set(Some(count));
        }

        let rows = convert_records_to_rows(page.records(), self.advanced_mode.clone());
        self.pages.set(pages);

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
        let loading_message = self.loading_message.get();
        let (has_records, loaded_count, column_count) = self
            .records
            .with_ref(|t| (!t.rows.is_empty(), t.rows.len(), t.columns.len()));
        let records_state = self.records_loading.get();
        let total_count = self.total_count.get();

        page! {
            box_ (width: fill, height: fill) {
                column (padding: (1, 2), gap: 1, height: fill, width: fill) style (bg: background) {
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

                if let Some(msg) = loading_message {
                    { loading_overlay("loading-overlay", &msg) }
                }
            }
        }
    }
}
