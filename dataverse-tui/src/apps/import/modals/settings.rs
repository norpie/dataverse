//! Import settings modal for configuring batch size and entity.

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{Autocomplete, AutocompleteState, Button, NumberInput, NumberInputState, Text};

use crate::file_io::FileRow;

use super::super::io::{count_operation_types, parse_headers, ColumnInfo};

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
    /// Pre-parsed entity name from sheet (suggestion).
    #[state(skip)]
    suggested_entity: String,

    /// Available entity options: (entity_set, display_label).
    #[state(skip)]
    entity_options: Vec<(String, String)>,

    /// Parsed columns from file.
    #[state(skip)]
    columns: Vec<ColumnInfo>,

    /// Rows from file.
    #[state(skip)]
    rows: Vec<FileRow>,

    /// Primary key field name.
    #[state(skip)]
    primary_key_field: String,

    /// Entity autocomplete state.
    entity: AutocompleteState<String>,

    /// Batch size input state.
    batch_size: NumberInputState,
}

impl ImportSettingsModal {
    pub fn new(
        suggested_entity: String,
        entity_options: Vec<(String, String)>,
        columns: Vec<String>,
        rows: Vec<FileRow>,
        primary_key_field: String,
    ) -> Self {
        let parsed_columns = parse_headers(&columns);

        Self {
            suggested_entity,
            entity_options,
            columns: parsed_columns,
            rows,
            primary_key_field,
            ..Default::default()
        }
    }
}

#[modal_impl]
impl ImportSettingsModal {
    fn default_result(&self) -> Option<ImportSettings> {
        None
    }

    #[on_start]
    async fn on_start(&self, mx: &ModalContext<Option<ImportSettings>>) {
        // Initialize entity autocomplete with suggested value
        self.entity.set(
            AutocompleteState::new(self.entity_options.clone())
                .with_value(self.suggested_entity.clone()),
        );

        // Initialize batch size to 1000
        self.batch_size.set(
            NumberInputState::new(1000.0)
                .with_min(1.0)
                .with_max(1000.0)
                .integer(),
        );

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

    fn element(&self) -> Element {
        // Calculate operation counts
        let (create_count, upsert_count) =
            count_operation_types(&self.rows, &self.columns, &self.primary_key_field);

        let total_ops = create_count + upsert_count;

        // Calculate batch count based on current batch size
        let batch_size = self.batch_size.with_ref(|s| s.value() as usize).max(1);
        let batch_count = (total_ops + batch_size - 1) / batch_size;

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
                    )
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
