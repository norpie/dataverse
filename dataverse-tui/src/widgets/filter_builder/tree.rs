//! Tree rendering for filter nodes.

use dataverse_lib::model::Value;
use rafter::widgets::{TreeItem, TreeNode};
use tuidom::{Color, Element, Style};

use crate::formatting::format_value;

use super::types::{CondOp, FilterNode};

/// Type-safe key for filter tree nodes.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum FilterTreeKey {
    /// A filter group (identified by ID).
    Group(usize),
    /// A filter condition (identified by ID).
    Condition(usize),
}

impl ToString for FilterTreeKey {
    fn to_string(&self) -> String {
        match self {
            Self::Group(id) => format!("filter-group-{}", id),
            Self::Condition(id) => format!("filter-cond-{}", id),
        }
    }
}

/// A tree item for rendering filter nodes.
#[derive(Clone, Debug)]
pub enum FilterTreeItem {
    /// A filter group (AND/OR), optionally negated (NOT).
    Group {
        id: usize,
        is_and: bool,
        is_negated: bool,
    },
    /// A filter condition leaf.
    Condition {
        id: usize,
        field: String,
        operator: CondOp,
        value: Value,
    },
}

impl TreeItem for FilterTreeItem {
    type Key = FilterTreeKey;

    fn key(&self) -> FilterTreeKey {
        match self {
            Self::Group { id, .. } => FilterTreeKey::Group(*id),
            Self::Condition { id, .. } => FilterTreeKey::Condition(*id),
        }
    }

    fn render(&self) -> Element {
        match self {
            Self::Group {
                is_and, is_negated, ..
            } => {
                let label = match (*is_negated, *is_and) {
                    (false, true) => "AND",
                    (false, false) => "OR",
                    (true, true) => "NOT AND",
                    (true, false) => "NOT OR",
                };
                Element::text(label)
                    .style(Style::new().foreground(Color::var("accent")).bold())
                    .style_focused(Style::new().foreground(Color::var("text.inverted")).bold())
            }
            Self::Condition {
                field,
                operator,
                value,
                ..
            } => {
                if operator.has_value() {
                    Element::row()
                        .gap(1)
                        .child(
                            Element::text(field)
                                .style(Style::new().foreground(Color::var("secondary")))
                                .style_focused(
                                    Style::new().foreground(Color::var("text.inverted")),
                                ),
                        )
                        .child(
                            Element::text(operator.label())
                                .style(Style::new().foreground(Color::var("muted")))
                                .style_focused(
                                    Style::new().foreground(Color::var("text.inverted")),
                                ),
                        )
                        .child(
                            Element::text(&format_value(value).raw)
                                .style(Style::new().foreground(Color::var("primary")))
                                .style_focused(
                                    Style::new().foreground(Color::var("text.inverted")),
                                ),
                        )
                } else {
                    Element::row()
                        .gap(1)
                        .child(
                            Element::text(field)
                                .style(Style::new().foreground(Color::var("secondary")))
                                .style_focused(
                                    Style::new().foreground(Color::var("text.inverted")),
                                ),
                        )
                        .child(
                            Element::text(operator.label())
                                .style(Style::new().foreground(Color::var("muted")))
                                .style_focused(
                                    Style::new().foreground(Color::var("text.inverted")),
                                ),
                        )
                }
            }
        }
    }
}

/// Build tree nodes from a FilterNode.
pub fn build_tree(filter: &FilterNode) -> Vec<TreeNode<FilterTreeItem>> {
    build_filter_node(filter).into_iter().collect()
}

/// Recursively build a tree node from a FilterNode.
fn build_filter_node(node: &FilterNode) -> Option<TreeNode<FilterTreeItem>> {
    match node {
        FilterNode::Empty => None,
        FilterNode::Group {
            id,
            is_and,
            is_negated,
            children,
        } => {
            let child_nodes: Vec<TreeNode<FilterTreeItem>> =
                children.iter().filter_map(build_filter_node).collect();
            Some(TreeNode::branch(
                FilterTreeItem::Group {
                    id: *id,
                    is_and: *is_and,
                    is_negated: *is_negated,
                },
                child_nodes,
            ))
        }
        FilterNode::Condition {
            id,
            field,
            operator,
            value,
        } => Some(TreeNode::leaf(FilterTreeItem::Condition {
            id: *id,
            field: field.clone(),
            operator: *operator,
            value: value.clone(),
        })),
    }
}
