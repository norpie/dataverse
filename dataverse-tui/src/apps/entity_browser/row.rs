//! Record row types for the entity browser table.

use std::collections::HashMap;

use dataverse_lib::model::metadata::{AttributeMetadata, EntityMetadata};
use rafter::widgets::TableRow;
use tuidom::Element;

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
    pub fn new(id: String) -> Self {
        Self {
            id,
            cells: HashMap::new(),
        }
    }

    pub fn set_cell(&mut self, column: String, value: String) {
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
