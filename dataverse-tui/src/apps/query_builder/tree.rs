//! Tree node types and TreeItem implementation for the query builder.

use dataverse_lib::api::query::Direction;
use dataverse_lib::model::Value;
use rafter::widgets::{TreeItem, TreeNode};
use tuidom::Element;

use crate::formatting::format_value;

use super::data::{CondOp, FilterNode, QueryData, SortField};

/// The five fixed sections of the query tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Section {
    Entity,
    Select,
    Filter,
    OrderBy,
    Top,
}

impl Section {
    pub fn label(self) -> &'static str {
        match self {
            Self::Entity => "Entity",
            Self::Select => "Select",
            Self::Filter => "Filter",
            Self::OrderBy => "Order By",
            Self::Top => "Top",
        }
    }
}

/// A node in the query builder tree.
#[derive(Clone, Debug)]
pub enum QueryTreeNode {
    /// A section root (always present, cannot be deleted).
    Section { section: Section, count: usize },
    /// The selected entity value.
    EntityValue(String),
    /// A selected field in the Select section.
    SelectField { index: usize, name: String },
    /// A filter group (AND/OR).
    FilterGroup { id: usize, is_and: bool },
    /// A filter condition leaf.
    FilterCondition {
        id: usize,
        field: String,
        operator: CondOp,
        value: Value,
    },
    /// A sort field in the Order By section.
    SortItem(SortField),
    /// The Top value.
    TopValue(u32),
}

impl TreeItem for QueryTreeNode {
    type Key = String;

    fn key(&self) -> String {
        match self {
            Self::Section { section, .. } => format!("section-{:?}", section),
            Self::EntityValue(_) => "entity-value".to_string(),
            Self::SelectField { index, .. } => format!("select-{}", index),
            Self::FilterGroup { id, .. } => format!("filter-group-{}", id),
            Self::FilterCondition { id, .. } => format!("filter-cond-{}", id),
            Self::SortItem(sf) => format!("sort-{}", sf.id),
            Self::TopValue(_) => "top-value".to_string(),
        }
    }

    fn render(&self) -> Element {
        match self {
            Self::Section { section, count } => {
                let label = if *count > 0 {
                    format!("{} ({})", section.label(), count)
                } else {
                    section.label().to_string()
                };
                Element::text(&label)
            }
            Self::EntityValue(name) => Element::text(name),
            Self::SelectField { name, .. } => Element::text(name),
            Self::FilterGroup { is_and, .. } => {
                let label = if *is_and { "AND" } else { "OR" };
                Element::text(label)
            }
            Self::FilterCondition {
                field,
                operator,
                value,
                ..
            } => {
                let label = if operator.has_value() {
                    format!("{} {} {}", field, operator.label(), format_value(value).raw)
                } else {
                    format!("{} {}", field, operator.label())
                };
                Element::text(&label)
            }
            Self::SortItem(sf) => {
                let dir = match sf.direction {
                    Direction::Asc => "ASC",
                    Direction::Desc => "DESC",
                };
                Element::text(&format!("{} {}", sf.field, dir))
            }
            Self::TopValue(n) => Element::text(&n.to_string()),
        }
    }
}

/// Build tree nodes from QueryData.
pub fn build_tree(query: &QueryData) -> Vec<TreeNode<QueryTreeNode>> {
    let mut roots = Vec::with_capacity(5);

    // Entity section
    let entity_children = match &query.entity {
        Some(name) => vec![TreeNode::leaf(QueryTreeNode::EntityValue(name.clone()))],
        None => vec![],
    };
    roots.push(TreeNode::branch(
        QueryTreeNode::Section {
            section: Section::Entity,
            count: if query.entity.is_some() { 1 } else { 0 },
        },
        entity_children,
    ));

    // Select section
    let select_children = query
        .select
        .iter()
        .enumerate()
        .map(|(i, f)| {
            TreeNode::leaf(QueryTreeNode::SelectField {
                index: i,
                name: f.clone(),
            })
        })
        .collect();
    roots.push(TreeNode::branch(
        QueryTreeNode::Section {
            section: Section::Select,
            count: query.select.len(),
        },
        select_children,
    ));

    // Filter section
    let filter_children = build_filter_nodes(&query.filter);
    let filter_count = count_conditions(&query.filter);
    roots.push(TreeNode::branch(
        QueryTreeNode::Section {
            section: Section::Filter,
            count: filter_count,
        },
        filter_children,
    ));

    // Order By section
    let order_children = query
        .order_by
        .iter()
        .map(|sf| TreeNode::leaf(QueryTreeNode::SortItem(sf.clone())))
        .collect();
    roots.push(TreeNode::branch(
        QueryTreeNode::Section {
            section: Section::OrderBy,
            count: query.order_by.len(),
        },
        order_children,
    ));

    // Top section
    let top_children = match query.top {
        Some(n) => vec![TreeNode::leaf(QueryTreeNode::TopValue(n))],
        None => vec![],
    };
    roots.push(TreeNode::branch(
        QueryTreeNode::Section {
            section: Section::Top,
            count: if query.top.is_some() { 1 } else { 0 },
        },
        top_children,
    ));

    roots
}

/// Recursively build tree nodes from a FilterNode.
fn build_filter_nodes(node: &FilterNode) -> Vec<TreeNode<QueryTreeNode>> {
    match node {
        FilterNode::Empty => vec![],
        FilterNode::Group {
            id,
            is_and,
            children,
        } => {
            let child_nodes: Vec<TreeNode<QueryTreeNode>> = children
                .iter()
                .flat_map(|c| match c {
                    FilterNode::Group { .. } => build_filter_nodes(c),
                    FilterNode::Condition {
                        id,
                        field,
                        operator,
                        value,
                    } => vec![TreeNode::leaf(QueryTreeNode::FilterCondition {
                        id: *id,
                        field: field.clone(),
                        operator: *operator,
                        value: value.clone(),
                    })],
                    FilterNode::Empty => vec![],
                })
                .collect();
            vec![TreeNode::branch(
                QueryTreeNode::FilterGroup {
                    id: *id,
                    is_and: *is_and,
                },
                child_nodes,
            )]
        }
        FilterNode::Condition {
            id,
            field,
            operator,
            value,
        } => vec![TreeNode::leaf(QueryTreeNode::FilterCondition {
            id: *id,
            field: field.clone(),
            operator: *operator,
            value: value.clone(),
        })],
    }
}

/// Count the number of leaf conditions in a filter tree.
fn count_conditions(node: &FilterNode) -> usize {
    match node {
        FilterNode::Empty => 0,
        FilterNode::Condition { .. } => 1,
        FilterNode::Group { children, .. } => children.iter().map(count_conditions).sum(),
    }
}
