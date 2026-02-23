//! Core filter builder types.

use dataverse_lib::model::Value;
use serde::{Deserialize, Serialize};

/// Internal result for tree move operations.
enum MoveResult {
    /// The move was handled within this subtree.
    Handled,
    /// The node needs to be moved out of its current parent.
    MoveOut(FilterNode),
    /// The target was not found in this subtree.
    NotFound,
}

/// A node in the filter tree.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum FilterNode {
    /// A logical group (AND/OR) containing child nodes, optionally negated (NOT).
    Group {
        id: usize,
        is_and: bool,
        is_negated: bool,
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
    #[default]
    Empty,
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

/// Result returned by the condition editor modal.
#[derive(Clone, Debug)]
pub struct ConditionData {
    pub field: String,
    pub operator: CondOp,
    pub value: Value,
}

impl FilterNode {
    /// Toggle AND/OR on the group with the given ID.
    pub fn toggle_group(&mut self, target_id: usize) {
        if let Self::Group {
            id,
            is_and,
            children,
            ..
        } = self
        {
            if *id == target_id {
                *is_and = !*is_and;
            } else {
                for child in children {
                    child.toggle_group(target_id);
                }
            }
        }
    }

    /// Toggle NOT on the group with the given ID.
    pub fn toggle_negation(&mut self, target_id: usize) {
        if let Self::Group {
            id,
            is_negated,
            children,
            ..
        } = self
        {
            if *id == target_id {
                *is_negated = !*is_negated;
            } else {
                for child in children {
                    child.toggle_negation(target_id);
                }
            }
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

    /// Collect all group IDs in the filter tree.
    pub fn collect_group_ids(&self) -> Vec<usize> {
        let mut ids = Vec::new();
        self.collect_group_ids_into(&mut ids);
        ids
    }

    fn collect_group_ids_into(&self, ids: &mut Vec<usize>) {
        if let Self::Group { id, children, .. } = self {
            ids.push(*id);
            for child in children {
                child.collect_group_ids_into(ids);
            }
        }
    }

    /// Find the parent group ID of a condition or group by its ID.
    /// Returns None if the node is at the root level or not found.
    pub fn find_parent_group_id(&self, target_id: usize) -> Option<usize> {
        match self {
            Self::Group { id, children, .. } => {
                // Check if any direct child matches
                for child in children {
                    match child {
                        Self::Condition { id: child_id, .. } | Self::Group { id: child_id, .. } => {
                            if *child_id == target_id {
                                return Some(*id);
                            }
                        }
                        Self::Empty => {}
                    }
                }
                // Recurse into child groups
                for child in children {
                    if let Some(parent_id) = child.find_parent_group_id(target_id) {
                        return Some(parent_id);
                    }
                }
                None
            }
            _ => None,
        }
    }

    /// Count the number of leaf conditions in the filter tree.
    pub fn count_conditions(&self) -> usize {
        match self {
            Self::Empty => 0,
            Self::Condition { .. } => 1,
            Self::Group { children, .. } => children.iter().map(|c| c.count_conditions()).sum(),
        }
    }

    /// Move a node up in the tree. Swaps with previous sibling, moves into
    /// a previous sibling group (as last child), or moves out of the current
    /// group (placed before it in the grandparent).
    pub fn move_node_up(&mut self, target_id: usize) -> bool {
        if self.node_id() == Some(target_id) {
            return false;
        }
        match self.move_node_up_impl(target_id) {
            MoveResult::Handled => true,
            MoveResult::MoveOut(node) => {
                // At root level, can't move out further — put it back
                if let Self::Group { children, .. } = self {
                    children.insert(0, node);
                }
                false
            }
            MoveResult::NotFound => false,
        }
    }

    fn move_node_up_impl(&mut self, target_id: usize) -> MoveResult {
        let Self::Group { children, .. } = self else {
            return MoveResult::NotFound;
        };

        // Check direct children
        if let Some(idx) = children.iter().position(|c| c.node_id() == Some(target_id)) {
            if idx > 0 {
                // Previous sibling is a group → move into it as last child
                if matches!(children[idx - 1], Self::Group { .. }) {
                    let node = children.remove(idx);
                    if let Self::Group {
                        children: prev_children,
                        ..
                    } = &mut children[idx - 1]
                    {
                        prev_children.push(node);
                    }
                    return MoveResult::Handled;
                }
                // Otherwise swap with previous sibling
                children.swap(idx, idx - 1);
                return MoveResult::Handled;
            }
            // First child → move out to parent
            let node = children.remove(0);
            return MoveResult::MoveOut(node);
        }

        // Recurse into children
        let child_count = children.len();
        for i in 0..child_count {
            match children[i].move_node_up_impl(target_id) {
                MoveResult::Handled => return MoveResult::Handled,
                MoveResult::MoveOut(node) => {
                    // Insert before the child group that returned MoveOut
                    children.insert(i, node);
                    return MoveResult::Handled;
                }
                MoveResult::NotFound => {}
            }
        }
        MoveResult::NotFound
    }

    /// Move a node down in the tree. Swaps with next sibling, moves into
    /// a next sibling group (as first child), or moves out of the current
    /// group (placed after it in the grandparent).
    pub fn move_node_down(&mut self, target_id: usize) -> bool {
        if self.node_id() == Some(target_id) {
            return false;
        }
        match self.move_node_down_impl(target_id) {
            MoveResult::Handled => true,
            MoveResult::MoveOut(node) => {
                // At root level, can't move out further — put it back
                if let Self::Group { children, .. } = self {
                    children.push(node);
                }
                false
            }
            MoveResult::NotFound => false,
        }
    }

    fn move_node_down_impl(&mut self, target_id: usize) -> MoveResult {
        let Self::Group { children, .. } = self else {
            return MoveResult::NotFound;
        };

        // Check direct children
        if let Some(idx) = children.iter().position(|c| c.node_id() == Some(target_id)) {
            if idx + 1 < children.len() {
                // Next sibling is a group → move into it as first child
                if matches!(children[idx + 1], Self::Group { .. }) {
                    let node = children.remove(idx);
                    // After removal, the group that was at idx+1 is now at idx
                    if let Self::Group {
                        children: next_children,
                        ..
                    } = &mut children[idx]
                    {
                        next_children.insert(0, node);
                    }
                    return MoveResult::Handled;
                }
                // Otherwise swap with next sibling
                children.swap(idx, idx + 1);
                return MoveResult::Handled;
            }
            // Last child → move out to parent
            let node = children.remove(idx);
            return MoveResult::MoveOut(node);
        }

        // Recurse into children
        let child_count = children.len();
        for i in 0..child_count {
            match children[i].move_node_down_impl(target_id) {
                MoveResult::Handled => return MoveResult::Handled,
                MoveResult::MoveOut(node) => {
                    // Insert after the child group that returned MoveOut
                    children.insert(i + 1, node);
                    return MoveResult::Handled;
                }
                MoveResult::NotFound => {}
            }
        }
        MoveResult::NotFound
    }

    /// Get the ID of this node, if it has one.
    fn node_id(&self) -> Option<usize> {
        match self {
            Self::Condition { id, .. } | Self::Group { id, .. } => Some(*id),
            Self::Empty => None,
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
