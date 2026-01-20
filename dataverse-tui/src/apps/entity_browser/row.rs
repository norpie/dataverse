//! Record row types for the entity browser table.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use dataverse_lib::model::metadata::{AttributeMetadata, EntityMetadata};
use rafter::widgets::TableRow;
use tuidom::Element;

use crate::formatting::FormattedValue;

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
    cells: HashMap<String, FormattedValue>,
    advanced_mode: Arc<AtomicBool>,
}

impl RecordRow {
    pub fn new(id: String, advanced_mode: Arc<AtomicBool>) -> Self {
        Self {
            id,
            cells: HashMap::new(),
            advanced_mode,
        }
    }

    pub fn set_cell(&mut self, column: String, value: FormattedValue) {
        self.cells.insert(column, value);
    }
}

impl TableRow for RecordRow {
    type Key = String;

    fn key(&self) -> String {
        self.id.clone()
    }

    fn cell(&self, column_id: &str) -> Element {
        let text = match self.cells.get(column_id) {
            Some(cv) if self.advanced_mode.load(Ordering::Relaxed) => &cv.raw,
            Some(cv) => &cv.display,
            None => "",
        };
        Element::text(text)
    }
}
