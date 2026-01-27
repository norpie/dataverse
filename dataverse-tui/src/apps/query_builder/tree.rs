//! Tree node types and TreeItem implementation for the query builder.

use dataverse_lib::api::query::Direction;
use dataverse_lib::model::Value;
use rafter::widgets::{TreeItem, TreeNode};
use tuidom::{Color, Element, Style};

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

/// Type-safe key for query tree nodes.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum QueryTreeKey {
    /// A section root node.
    Section(Section),
    /// The selected entity value.
    EntityValue,
    /// A selected field (indexed by position).
    SelectField(usize),
    /// A filter group (identified by ID).
    FilterGroup(usize),
    /// A filter condition (identified by ID).
    FilterCondition(usize),
    /// A sort field (identified by ID).
    SortItem(usize),
    /// The Top value.
    TopValue,
}

impl ToString for QueryTreeKey {
    fn to_string(&self) -> String {
        match self {
            Self::Section(section) => format!("section-{:?}", section),
            Self::EntityValue => "entity-value".to_string(),
            Self::SelectField(index) => format!("select-{}", index),
            Self::FilterGroup(id) => format!("filter-group-{}", id),
            Self::FilterCondition(id) => format!("filter-cond-{}", id),
            Self::SortItem(id) => format!("sort-{}", id),
            Self::TopValue => "top-value".to_string(),
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
    type Key = QueryTreeKey;

    fn key(&self) -> QueryTreeKey {
        match self {
            Self::Section { section, .. } => QueryTreeKey::Section(*section),
            Self::EntityValue(_) => QueryTreeKey::EntityValue,
            Self::SelectField { index, .. } => QueryTreeKey::SelectField(*index),
            Self::FilterGroup { id, .. } => QueryTreeKey::FilterGroup(*id),
            Self::FilterCondition { id, .. } => QueryTreeKey::FilterCondition(*id),
            Self::SortItem(sf) => QueryTreeKey::SortItem(sf.id),
            Self::TopValue(_) => QueryTreeKey::TopValue,
        }
    }

    fn render(&self) -> Element {
        match self {
            Self::Section { section, count } => {
                if *count > 0 {
                    Element::row()
                        .gap(1)
                        .child(
                            Element::text(section.label())
                                .style(Style::new().foreground(Color::var("interact")).bold())
                                .style_focused(
                                    Style::new().foreground(Color::var("text.inverted")).bold(),
                                ),
                        )
                        .child(
                            Element::text(&format!("({})", count))
                                .style(Style::new().foreground(Color::var("muted")))
                                .style_focused(
                                    Style::new().foreground(Color::var("text.inverted")),
                                ),
                        )
                } else {
                    Element::text(section.label())
                        .style(Style::new().foreground(Color::var("interact")).bold())
                        .style_focused(Style::new().foreground(Color::var("text.inverted")).bold())
                }
            }
            Self::EntityValue(name) => Element::text(name)
                .style(Style::new().foreground(Color::var("primary")))
                .style_focused(Style::new().foreground(Color::var("text.inverted"))),
            Self::SelectField { name, .. } => Element::text(name)
                .style(Style::new().foreground(Color::var("secondary")))
                .style_focused(Style::new().foreground(Color::var("text.inverted"))),
            Self::FilterGroup { is_and, .. } => {
                let label = if *is_and { "AND" } else { "OR" };
                Element::text(label)
                    .style(Style::new().foreground(Color::var("accent")).bold())
                    .style_focused(Style::new().foreground(Color::var("text.inverted")).bold())
            }
            Self::FilterCondition {
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
            Self::SortItem(sf) => {
                let dir = match sf.direction {
                    Direction::Asc => "asc",
                    Direction::Desc => "desc",
                };
                Element::row()
                    .gap(1)
                    .child(
                        Element::text(&sf.field)
                            .style(Style::new().foreground(Color::var("secondary")))
                            .style_focused(Style::new().foreground(Color::var("text.inverted"))),
                    )
                    .child(
                        Element::text(dir)
                            .style(Style::new().foreground(Color::var("muted")))
                            .style_focused(Style::new().foreground(Color::var("text.inverted"))),
                    )
            }
            Self::TopValue(n) => Element::text(&n.to_string())
                .style(Style::new().foreground(Color::var("primary")))
                .style_focused(Style::new().foreground(Color::var("text.inverted"))),
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
    let filter_children = build_filter_node(&query.filter).into_iter().collect();
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

/// Recursively build a tree node from a FilterNode.
fn build_filter_node(node: &FilterNode) -> Option<TreeNode<QueryTreeNode>> {
    match node {
        FilterNode::Empty => None,
        FilterNode::Group {
            id,
            is_and,
            children,
        } => {
            let child_nodes: Vec<TreeNode<QueryTreeNode>> = children
                .iter()
                .filter_map(|c| build_filter_node(c))
                .collect();
            Some(TreeNode::branch(
                QueryTreeNode::FilterGroup {
                    id: *id,
                    is_and: *is_and,
                },
                child_nodes,
            ))
        }
        FilterNode::Condition {
            id,
            field,
            operator,
            value,
        } => Some(TreeNode::leaf(QueryTreeNode::FilterCondition {
            id: *id,
            field: field.clone(),
            operator: *operator,
            value: value.clone(),
        })),
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
