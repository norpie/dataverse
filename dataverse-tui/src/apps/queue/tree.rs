//! Tree item implementation for the queue.

use std::collections::HashMap;

use chrono::Utc;
use dataverse_lib::api::BatchItem;
use dataverse_lib::api::Operation;
use rafter::widgets::TreeItem;
use tuidom::{Color, Element, Style};
use uuid::Uuid;

use super::types::{ItemTiming, QueueItem, QueuePayload};

/// A node in the queue tree.
#[derive(Clone, Debug)]
pub enum QueueTreeNode {
    /// A top-level queue item.
    Item {
        item: QueueItem,
        timing: Option<ItemTiming>,
    },
    /// A child operation within a batch item.
    Operation {
        parent_id: i64,
        index: usize,
        label: String,
        operation: Operation,
    },
}

impl TreeItem for QueueTreeNode {
    type Key = String;

    fn key(&self) -> String {
        match self {
            Self::Item { item, .. } => format!("item-{}", item.id),
            Self::Operation {
                parent_id, index, ..
            } => format!("item-{}-op-{}", parent_id, index),
        }
    }

    fn render(&self) -> Element {
        match self {
            Self::Item { item, timing } => {
                let priority_text = format!("{}", item.priority);
                let dot_color = item.status.color();

                let mut row = Element::row()
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
                    .child(Element::text(&item.description));

                // Add timing information if available
                if let Some(timing) = timing {
                    let timing_text = match timing {
                        ItemTiming::Running { started_at } => {
                            let elapsed = Utc::now() - *started_at;
                            format_duration(elapsed.num_milliseconds())
                        }
                        ItemTiming::Completed { duration_ms } => format_duration(*duration_ms),
                    };
                    row = row.child(
                        Element::text(&timing_text)
                            .style(Style::new().foreground(Color::var("muted"))),
                    );
                }

                row
            }
            Self::Operation { label, .. } => Element::row()
                .child(Element::text(label).style(Style::new().foreground(Color::var("muted")))),
        }
    }
}

/// Format duration in milliseconds as a human-readable string.
fn format_duration(ms: i64) -> String {
    if ms < 1000 {
        format!("{}ms", ms)
    } else if ms < 60_000 {
        format!("{:.1}s", ms as f64 / 1000.0)
    } else if ms < 3_600_000 {
        let mins = ms / 60_000;
        let secs = (ms % 60_000) / 1000;
        format!("{}m{}s", mins, secs)
    } else {
        let hours = ms / 3_600_000;
        let mins = (ms % 3_600_000) / 60_000;
        format!("{}h{}m", hours, mins)
    }
}

/// Format an operation as a short label.
pub fn format_operation(op: &Operation) -> String {
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

/// Convert queue items into tree nodes with timing information.
pub fn build_tree_nodes(
    items: &[QueueItem],
    timing_map: &HashMap<i64, ItemTiming>,
) -> Vec<rafter::widgets::TreeNode<QueueTreeNode>> {
    items
        .iter()
        .map(|item| build_item_node(item, timing_map.get(&item.id).copied()))
        .collect()
}

fn build_item_node(
    item: &QueueItem,
    timing: Option<ItemTiming>,
) -> rafter::widgets::TreeNode<QueueTreeNode> {
    let node = QueueTreeNode::Item {
        item: item.clone(),
        timing,
    };

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
                            operation: op.clone(),
                        }));
                    }
                    BatchItem::Changeset(cs) => {
                        for (cs_idx, op) in cs.operations().iter().enumerate() {
                            children.push(rafter::widgets::TreeNode::leaf(
                                QueueTreeNode::Operation {
                                    parent_id: item.id,
                                    index: index * 1000 + cs_idx,
                                    label: format!("[tx] {}", format_operation(op)),
                                    operation: op.clone(),
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
