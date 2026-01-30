//! Import settings modal for configuring batch size and entity.


use dataverse_lib::DataverseClient;
use dataverse_lib::error::Error as DataverseError;
use dataverse_lib::model::Entity;
use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{
    Autocomplete, AutocompleteState, Button, NumberInput, NumberInputState, Text,
};

use crate::file_io::FileRow;
use crate::modals::LoadingModal;

use super::super::io::{ColumnInfo, count_operation_types, parse_headers};

/// Result of the import settings modal.
pub struct ImportSettings {
    /// Selected entity set name.
    pub entity_set: String,
    /// Batch size (1-1000).
    pub batch_size: usize,
}

/// Modal for configuring import settings.
#[modal(size = Md)]
pub struct ImportSettingsModal {
    /// Dataverse client for fetching metadata.
    #[state(skip)]
    client: DataverseClient,

    /// Pre-parsed entity name from sheet (suggestion).
    #[state(skip)]
    suggested_entity: Option<String>,

    /// Available entity options: (entity_set, display_label).
    #[state(skip)]
    entity_options: Vec<(String, String)>,

    /// Parsed columns from file.
    #[state(skip)]
    columns: Vec<ColumnInfo>,

    /// Rows from file.
    #[state(skip)]
    rows: Vec<FileRow>,

    /// Primary key field name (reactively updated).
    primary_key_field: Option<String>,

    /// Entity autocomplete state.
    entity: AutocompleteState<String>,

    /// Batch size input state.
    batch_size: NumberInputState,
}

impl ImportSettingsModal {
    pub fn with_config(
        client: DataverseClient,
        suggested_entity: Option<String>,
        entity_options: Vec<(String, String)>,
        columns: Vec<String>,
        rows: Vec<FileRow>,
    ) -> Self {
        let parsed_columns = parse_headers(&columns);

        Self::new(
            client,
            suggested_entity,
            entity_options,
            parsed_columns,
            rows,
            None,
            AutocompleteState::default(),
            NumberInputState::default(),
        )
    }
}

#[modal_impl]
impl ImportSettingsModal {
    fn default_result(&self) -> Option<ImportSettings> {
        None
    }

    #[on_start]
    async fn on_start(&self, gx: &GlobalContext, mx: &ModalContext<Option<ImportSettings>>) {
        // Initialize entity autocomplete with suggested value (if any)
        let mut autocomplete = AutocompleteState::new(self.entity_options.clone());
        if let Some(ref entity) = self.suggested_entity {
            autocomplete = autocomplete.with_value(entity.clone());
        }
        self.entity.set(autocomplete);

        // Initialize batch size to 1000
        self.batch_size.set(
            NumberInputState::new(1000.0)
                .with_min(1.0)
                .with_max(1000.0)
                .integer(),
        );

        // If we have a suggested entity, fetch its primary key immediately
        if let Some(ref entity_set) = self.suggested_entity {
            self.fetch_primary_key(gx, entity_set).await;
        }

        mx.focus("entity-autocomplete");
    }

    #[keybinds]
    fn keys() {
        bind("escape", cancel);
    }

    #[handler]
    async fn cancel(&self, mx: &ModalContext<Option<ImportSettings>>) {
        mx.close(None);
    }

    #[handler]
    async fn on_entity_change(&self, gx: &GlobalContext) {
        log::debug!("on_entity_change called");
        let entity_set = self.entity.with_ref(|s| s.value().cloned());

        log::debug!("Selected entity: {:?}", entity_set);

        if let Some(entity_set) = entity_set {
            self.fetch_primary_key(gx, &entity_set).await;
        } else {
            // No entity selected, clear primary key
            self.primary_key_field.set(None);
        }
    }

    /// Fetch primary key for the given entity.
    async fn fetch_primary_key(&self, gx: &GlobalContext, entity_set: &str) {
        log::debug!("Fetching primary key for entity: {}", entity_set);
        let client = self.client.clone();
        let entity = Entity::set(entity_set.to_string());

        // Fetch entity metadata to get primary_id_attribute
        let entity_result = gx
            .modal(LoadingModal::run_with_default(
                "Fetching metadata...",
                || Err(DataverseError::Cancelled),
                async move { client.metadata().entity(entity).await },
            ))
            .await;

        log::debug!(
            "Metadata fetch result: {:?}",
            entity_result.as_ref().map(|e| &e.primary_id_attribute)
        );

        match entity_result {
            Ok(entity_metadata) => {
                let pk = entity_metadata.primary_id_attribute.clone();
                log::debug!("Found primary key: {}", pk);
                self.primary_key_field.set(Some(pk));
            }
            Err(e) if e.is_cancelled() => {
                self.primary_key_field.set(None);
            }
            Err(e) => {
                gx.toast(Toast::error(format!("Failed to fetch metadata: {}", e)));
                self.primary_key_field.set(None);
            }
        }
    }

    #[handler]
    async fn confirm(&self, mx: &ModalContext<Option<ImportSettings>>) {
        let entity_set = self.entity.with_ref(|s| s.value().cloned());

        if let Some(entity_set) = entity_set {
            let batch_size = self.batch_size.with_ref(|s| s.value() as usize);

            mx.close(Some(ImportSettings {
                entity_set,
                batch_size,
            }));
        }
    }

    #[derived]
    fn operation_counts(&self) -> (usize, usize) {
        let primary_key = self.primary_key_field.get().unwrap_or_default();
        log::debug!(
            "operation_counts: primary_key={:?}, rows={}, columns={}",
            primary_key,
            self.rows.len(),
            self.columns.len()
        );
        let counts = count_operation_types(&self.rows, &self.columns, &primary_key);
        log::debug!(
            "operation_counts result: creates={}, upserts={}",
            counts.0,
            counts.1
        );
        counts
    }

    #[derived]
    fn batch_count(&self) -> usize {
        let (create_count, upsert_count) = self.operation_counts();
        let total_ops = create_count + upsert_count;
        let batch_size = self.batch_size.with_ref(|s| s.value() as usize).max(1);
        total_ops.div_ceil(batch_size)
    }

    fn element(&self) -> Element {
        let (create_count, upsert_count) = self.operation_counts();
        let batch_count = self.batch_count();

        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                // Title
                text (content: "Import Settings") style (bold, fg: interact)

                // Entity selection
                column (gap: 1) {
                    text (content: "Entity") style (fg: primary)
                    autocomplete (
                        state: self.entity,
                        id: "entity-autocomplete",
                        placeholder: "Select entity..."
                    ) on_select: on_entity_change()
                }

                // Batch size
                column (gap: 1) {
                    text (content: "Batch Size (1-1000)") style (fg: primary)
                    number_input (
                        state: self.batch_size,
                        id: "batch-size",
                        placeholder: "1000",
                        width: 10
                    )
                }

                // Preview section
                column (gap: 1) {
                    text (content: "Preview") style (fg: primary)
                    row (gap: 2) {
                        text (content: {format!("Rows: {}", self.rows.len())}) style (fg: muted)
                    }
                    row (gap: 2) {
                        text (content: {format!("Creates: {}", create_count)}) style (fg: muted)
                    }
                    row (gap: 2) {
                        text (content: {format!("Upserts: {}", upsert_count)}) style (fg: muted)
                    }
                    row (gap: 2) {
                        text (content: {format!("→ {} batches", batch_count)}) style (fg: primary)
                    }
                }

                // Buttons
                row (width: fill, justify: between) {
                    button (label: "Cancel", hint: "esc", id: "cancel") on_activate: cancel()
                    button (label: "Import", id: "import") on_activate: confirm()
                }
            }
        }
    }
}
