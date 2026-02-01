//! Reusable filter builder components.
//!
//! This module provides types and utilities for building OData filter expressions
//! with a tree-based UI.
//!
//! # Components
//!
//! - [`FilterNode`] - The core filter tree data structure
//! - [`CondOp`] - Filter comparison operators
//! - [`ConditionData`] - Result from the condition editor modal
//! - [`FilterTreeItem`] - Tree item implementation for rendering filters
//! - [`ConditionEditorModal`] - Modal for creating/editing conditions
//! - [`convert_filter`] - Convert FilterNode to OData Filter
//!
//! # Example
//!
//! ```ignore
//! use crate::widgets::filter_builder::{FilterNode, FilterTreeItem, build_tree};
//!
//! // In app state:
//! filter: FilterNode,
//! tree_state: TreeState<FilterTreeItem>,
//!
//! // Build tree nodes from filter
//! let nodes = build_tree(&self.filter);
//! self.tree_state.set_roots(nodes);
//! ```

mod condition_editor;
mod convert;
mod tree;
mod types;

pub use condition_editor::{
    ConditionEditorModal, operators_for_type, parse_value, type_hint_text,
};
pub use convert::{ConvertError, convert_filter};
pub use tree::{FilterTreeItem, FilterTreeKey, build_tree};
pub use types::{CondOp, ConditionData, FilterNode};
