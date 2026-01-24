use dataverse_lib::api::query::Direction;
use dataverse_lib::model::Value;
use serde::{Deserialize, Serialize};

/// The complete query being built.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QueryData {
    /// Entity set name (e.g., "accounts").
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

impl FilterNode {
    /// Toggle AND/OR on the group with the given ID.
    pub fn toggle_group(&mut self, target_id: usize) {
        match self {
            Self::Group {
                id,
                is_and,
                children,
            } => {
                if *id == target_id {
                    *is_and = !*is_and;
                } else {
                    for child in children {
                        child.toggle_group(target_id);
                    }
                }
            }
            _ => {}
        }
    }

    /// Remove a node (condition or group) by ID. Returns true if removed.
    pub fn remove_node(&mut self, target_id: usize) -> bool {
        match self {
            Self::Group { children, .. } => {
                let len_before = children.len();
                children.retain(|child| match child {
                    Self::Condition { id, .. } => *id != target_id,
                    Self::Group { id, .. } => *id != target_id,
                    Self::Empty => true,
                });
                if children.len() < len_before {
                    // If the root group is now empty, become Empty
                    if children.is_empty() {
                        *self = Self::Empty;
                    }
                    return true;
                }
                // Recurse into children
                for child in children {
                    if child.remove_node(target_id) {
                        return true;
                    }
                }
                false
            }
            _ => false,
        }
    }

    /// Add a child node to the group with the given ID. Returns true if added.
    pub fn add_to_group(&mut self, target_id: usize, node: FilterNode) -> bool {
        match self {
            Self::Group { id, children, .. } => {
                if *id == target_id {
                    children.push(node);
                    return true;
                }
                for child in children {
                    if child.add_to_group(target_id, node.clone()) {
                        return true;
                    }
                }
                false
            }
            _ => false,
        }
    }

    /// Find a condition by ID and return its data.
    pub fn find_condition(&self, target_id: usize) -> Option<(String, CondOp, Value)> {
        match self {
            Self::Condition {
                id,
                field,
                operator,
                value,
            } => {
                if *id == target_id {
                    Some((field.clone(), *operator, value.clone()))
                } else {
                    None
                }
            }
            Self::Group { children, .. } => {
                children.iter().find_map(|c| c.find_condition(target_id))
            }
            Self::Empty => None,
        }
    }

    /// Update a condition in place by ID. Returns true if updated.
    pub fn update_condition(
        &mut self,
        target_id: usize,
        new_field: String,
        new_op: CondOp,
        new_value: Value,
    ) -> bool {
        match self {
            Self::Condition {
                id,
                field,
                operator,
                value,
            } => {
                if *id == target_id {
                    *field = new_field;
                    *operator = new_op;
                    *value = new_value;
                    true
                } else {
                    false
                }
            }
            Self::Group { children, .. } => children.iter_mut().any(|c| {
                c.update_condition(target_id, new_field.clone(), new_op, new_value.clone())
            }),
            Self::Empty => false,
        }
    }

    /// Check if the group with the given ID has any children.
    pub fn group_has_children(&self, target_id: usize) -> bool {
        match self {
            Self::Group { id, children, .. } => {
                if *id == target_id {
                    !children.is_empty()
                } else {
                    children.iter().any(|c| c.group_has_children(target_id))
                }
            }
            _ => false,
        }
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
