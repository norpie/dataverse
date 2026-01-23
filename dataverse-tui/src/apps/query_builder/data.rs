use dataverse_lib::api::query::Direction;
use dataverse_lib::model::Value;
use serde::{Deserialize, Serialize};

/// The complete query being built.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QueryData {
    pub entity: Option<String>,
    pub select: Vec<String>,
    pub filter: FilterNode,
    pub order_by: Vec<SortField>,
    pub top: Option<u32>,
    next_id: usize,
}

/// A node in the filter tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FilterNode {
    /// A logical group (AND/OR) containing child nodes.
    Group {
        id: usize,
        is_and: bool,
        children: Vec<FilterNode>,
    },
    /// A leaf condition.
    Condition {
        id: usize,
        field: String,
        operator: CondOp,
        value: Value,
    },
    /// Empty root (no filter defined yet).
    Empty,
}

impl Default for FilterNode {
    fn default() -> Self {
        Self::Empty
    }
}

/// Comparison operator for a filter condition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CondOp {
    Eq,
    Ne,
    Gt,
    Ge,
    Lt,
    Le,
    Contains,
    StartsWith,
    EndsWith,
    IsNull,
    IsNotNull,
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

impl CondOp {
    /// Display label for the operator.
    pub fn label(self) -> &'static str {
        match self {
            Self::Eq => "eq",
            Self::Ne => "ne",
            Self::Gt => "gt",
            Self::Ge => "ge",
            Self::Lt => "lt",
            Self::Le => "le",
            Self::Contains => "contains",
            Self::StartsWith => "startswith",
            Self::EndsWith => "endswith",
            Self::IsNull => "is null",
            Self::IsNotNull => "is not null",
        }
    }

    /// Whether this operator takes a value (IsNull/IsNotNull don't).
    pub fn has_value(self) -> bool {
        !matches!(self, Self::IsNull | Self::IsNotNull)
    }
}
