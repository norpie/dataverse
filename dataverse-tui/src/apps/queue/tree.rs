//! Tree item implementation for the queue.

use dataverse_lib::api::BatchItem;
use dataverse_lib::api::Operation;
use rafter::widgets::TreeItem;
use tuidom::{Color, Element, Style};
use uuid::Uuid;

use super::types::{QueueItem, QueuePayload};

/// A node in the queue tree.
#[derive(Clone, Debug)]
pub enum QueueTreeNode {
    /// A top-level queue item.
    Item(QueueItem),
    /// A child operation within a batch item.
    Operation {
        parent_id: i64,
        index: usize,
        label: String,
    },
}

impl TreeItem for QueueTreeNode {
    type Key = String;

    fn key(&self) -> String {
        match self {
            Self::Item(item) => format!("item-{}", item.id),
            Self::Operation {
                parent_id, index, ..
            } => format!("item-{}-op-{}", parent_id, index),
        }
    }

    fn render(&self) -> Element {
        match self {
            Self::Item(item) => {
                let priority_text = format!("{}", item.priority);
                let dot_color = item.status.color();

                Element::row()
                    .gap(1)
                    .child(
                        Element::text(&priority_text)
                            .style(Style::new().foreground(Color::var("muted"))),
                    )
                    .child(Element::text("●").style(Style::new().foreground(Color::var(dot_color))))
                    .child(
                        Element::text(&item.source)
                            .style(Style::new().foreground(Color::var("muted"))),
                    )
                    .child(Element::text(&item.description))
            }
            Self::Operation { label, .. } => Element::row()
                .child(Element::text(label).style(Style::new().foreground(Color::var("muted")))),
        }
    }
}

/// Format an operation as a short label.
fn format_operation(op: &Operation) -> String {
    match op {
        Operation::Create { entity, .. } => format!("Create {}", entity.set_name()),
        Operation::Retrieve { entity, id, .. } => {
            format!("Retrieve {} {}", entity.set_name(), short_id(id))
        }
        Operation::Update { entity, id, .. } => {
            format!("Update {} {}", entity.set_name(), short_id(id))
        }
        Operation::Delete { entity, id, .. } => {
            format!("Delete {} {}", entity.set_name(), short_id(id))
        }
        Operation::Upsert { entity, id, .. } => {
            format!("Upsert {} {}", entity.set_name(), short_id(id))
        }
        Operation::Associate {
            entity,
            relationship,
            ..
        } => format!("Associate {} ({})", entity.set_name(), relationship),
        Operation::Disassociate {
            entity,
            relationship,
            ..
        } => format!("Disassociate {} ({})", entity.set_name(), relationship),
        Operation::SetLookup {
            entity,
            nav_property,
            id,
            ..
        } => format!(
            "SetLookup {} {} {}",
            entity.set_name(),
            nav_property,
            short_id(id)
        ),
        Operation::ClearLookup {
            entity,
            nav_property,
            id,
            ..
        } => format!(
            "ClearLookup {} {} {}",
            entity.set_name(),
            nav_property,
            short_id(id)
        ),
    }
}

/// Shorten a UUID to first 8 characters.
fn short_id(id: &Uuid) -> String {
    let s = id.to_string();
    s[..8].to_string()
}

/// Convert queue items into tree nodes.
pub fn build_tree_nodes(items: &[QueueItem]) -> Vec<rafter::widgets::TreeNode<QueueTreeNode>> {
    items.iter().map(|item| build_item_node(item)).collect()
}

fn build_item_node(item: &QueueItem) -> rafter::widgets::TreeNode<QueueTreeNode> {
    let node = QueueTreeNode::Item(item.clone());

    match &item.payload {
        QueuePayload::Single(_) => rafter::widgets::TreeNode::leaf(node),
        QueuePayload::Batch(batch) => {
            let mut children = Vec::new();
            for (index, batch_item) in batch.items().iter().enumerate() {
                match batch_item {
                    BatchItem::Operation(op) => {
                        children.push(rafter::widgets::TreeNode::leaf(QueueTreeNode::Operation {
                            parent_id: item.id,
                            index,
                            label: format_operation(op),
                        }));
                    }
                    BatchItem::Changeset(cs) => {
                        for (cs_idx, op) in cs.operations().iter().enumerate() {
                            children.push(rafter::widgets::TreeNode::leaf(
                                QueueTreeNode::Operation {
                                    parent_id: item.id,
                                    index: index * 1000 + cs_idx,
                                    label: format!("[tx] {}", format_operation(op)),
                                },
                            ));
                        }
                    }
                }
            }
            rafter::widgets::TreeNode::branch(node, children)
        }
    }
}
