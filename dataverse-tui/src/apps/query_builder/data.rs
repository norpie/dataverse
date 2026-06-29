use dataverse_lib::api::query::Direction;
use dataverse_lib::model::Entity;
use serde::{Deserialize, Serialize};

// Re-export filter types from the shared module
pub use crate::widgets::filter_builder::FilterNode;

/// The complete query being built.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QueryData {
    /// Selected entity for the query.
    pub entity: Option<Entity>,
    pub select: Vec<String>,
    pub filter: FilterNode,
    pub order_by: Vec<SortField>,
    pub top: Option<u32>,
    next_id: usize,
}

/// A sort field with a unique ID for tree tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SortField {
    pub id: usize,
    pub field: String,
    pub direction: Direction,
}

impl QueryData {
    pub fn new() -> Self {
        Self::default()
    }

    /// Allocate a unique ID for a new filter node or sort field.
    pub fn next_id(&mut self) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
}
