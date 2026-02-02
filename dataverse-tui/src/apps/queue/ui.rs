//! Queue app UI rendering helpers.

use std::collections::VecDeque;

use dataverse_lib::api::{BatchItem, Operation};
use rafter::element;
use rafter::prelude::*;
use rafter::widgets::{Text, TreeItem};
use tuidom::Color;

use crate::formatting::format_value;

use super::Queue;
use super::repository::StatusCounts;
use super::tree::{QueueTreeNode, format_operation};
use super::types::{QueueItem, QueuePayload};

/// Recursively find a node by key in the tree.
fn find_node_recursive(
    node: &rafter::widgets::TreeNode<QueueTreeNode>,
    key: &str,
) -> Option<QueueTreeNode> {
    if TreeItem::key(&node.value) == key {
        return Some(node.value.clone());
    }
    for child in &node.children {
        if let Some(found) = find_node_recursive(child, key) {
            return Some(found);
        }
    }
    None
}

impl Queue {
    /// Get the currently focused queue item from the tree state.
    pub(super) fn focused_item(&self) -> Option<QueueItem> {
        self.tree_state.with_ref(|s| {
            s.focused_key.as_ref().and_then(|key| {
                let item_id: Option<i64> = key
                    .strip_prefix("item-")
                    .and_then(|rest| rest.split('-').next())
                    .and_then(|id_str| id_str.parse().ok());
                item_id.and_then(|id| {
                    s.roots.iter().find_map(|node| {
                        if let QueueTreeNode::Item { item, .. } = &node.value
                            && item.id == id
                        {
                            return Some(item.clone());
                        }
                        None
                    })
                })
            })
        })
    }

    pub(super) fn render_preview(&self) -> Element {
        let focused_key = self.tree_state.with_ref(|s| s.focused_key.clone());

        let Some(key) = focused_key else {
            return element! {
                column (padding: (1, 2), width: fill, height: fill) style (bg: surface)
            };
        };

        // Check if this is an operation key (contains "-op-")
        if key.contains("-op-") {
            // Find the operation node in the tree
            let node = self.tree_state.with_ref(|s| {
                s.roots
                    .iter()
                    .find_map(|root| find_node_recursive(root, &key))
            });

            if let Some(QueueTreeNode::Operation {
                operation, label, ..
            }) = node
            {
                return render_operation_preview(&operation, &label);
            }
        }

        // Otherwise, find the item from the tree
        let item = self.tree_state.with_ref(|s| {
            // Extract item ID from key
            let item_id: Option<i64> = key
                .strip_prefix("item-")
                .and_then(|rest| rest.split('-').next())
                .and_then(|id_str| id_str.parse().ok());

            item_id.and_then(|id| {
                // Search roots for the matching item
                s.roots.iter().find_map(|node| {
                    if let QueueTreeNode::Item { item, .. } = &node.value
                        && item.id == id
                    {
                        return Some(item.clone());
                    }
                    None
                })
            })
        });

        let Some(item) = item else {
            return element! {
                column (padding: (1, 2), width: fill, height: fill) style (bg: surface)
            };
        };

        let status_text = format!("{:?}", item.status);
        let status_color = Color::var(item.status.color());
        let created = item.created_at.format("%Y-%m-%d %H:%M").to_string();
        let priority_text = item.priority.to_string();
        let ops_text = format!("{} operation(s)", item.payload.operation_count());

        // Build operation list for batch items
        let mut op_elements = vec![];
        if let QueuePayload::Batch(batch) = &item.payload {
            for (index, batch_item) in batch.items().iter().enumerate() {
                match batch_item {
                    BatchItem::Operation(op) => {
                        let op_label = format_operation(op);
                        op_elements.push(element! {
                            row (gap: 1) {
                                text (content: {format!("{}.", index + 1)}) style (fg: muted)
                                text (content: {op_label})
                            }
                        });
                    }
                    BatchItem::Changeset(cs) => {
                        for (_cs_idx, op) in cs.operations().iter().enumerate() {
                            let op_label = format_operation(op);
                            let label_with_tx = format!("[tx] {}", op_label);
                            op_elements.push(element! {
                                row (gap: 1) {
                                    text (content: {format!("{}.", index + 1)}) style (fg: muted)
                                    text (content: {label_with_tx})
                                }
                            });
                        }
                    }
                }
            }
        }

        element! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                text (content: {item.description.clone()}) style (bold, fg: primary)

                row (gap: 1) {
                    text (content: "●") style (fg: {status_color})
                    text (content: {status_text})
                }
                row (gap: 1) {
                    text (content: "source") style (fg: muted)
                    text (content: {item.source.clone()})
                }
                row (gap: 1) {
                    text (content: "priority") style (fg: muted)
                    text (content: {priority_text})
                }
                row (gap: 1) {
                    text (content: "created") style (fg: muted)
                    text (content: {created})
                }
                row (gap: 1) {
                    text (content: "ops") style (fg: muted)
                    text (content: {ops_text})
                }

                    text (content: "Operations:") style (fg: interact)
                    ...op_elements
            }
        }
    }
}

/// Format the estimated time remaining based on recent execution durations.
pub fn format_eta(durations: &VecDeque<i64>, counts: &StatusCounts) -> String {
    if durations.is_empty() {
        return String::new();
    }

    let remaining = counts.ready + counts.paused;
    if remaining == 0 {
        return String::new();
    }

    let avg_ms: i64 = durations.iter().sum::<i64>() / durations.len() as i64;
    let total_ms = avg_ms * remaining as i64;
    let total_secs = total_ms / 1000;

    if total_secs < 60 {
        format!("~{}s", total_secs)
    } else if total_secs < 3600 {
        format!("~{}m", total_secs / 60)
    } else {
        format!("~{}h{}m", total_secs / 3600, (total_secs % 3600) / 60)
    }
}

/// Render preview for an individual operation.
fn render_operation_preview(operation: &Operation, label: &str) -> Element {
    match operation {
        Operation::Create { entity, record, .. } => {
            let label_str = label.to_string();
            let entity_name = entity.set_name().to_string();
            let mut field_elements = vec![];

            for (key, value) in record.fields().iter() {
                let key_str = key.clone();
                let formatted = format_value(value);
                let display_str = formatted.display;
                field_elements.push(element! {
                    row (gap: 1) {
                        text (content: {key_str}) style (fg: muted)
                        text (content: {display_str})
                    }
                });
            }

            element! {
                column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                    text (content: {label_str}) style (bold, fg: primary)

                    row (gap: 1) {
                        text (content: "type") style (fg: muted)
                        text (content: "Create")
                    }
                    row (gap: 1) {
                        text (content: "entity") style (fg: muted)
                        text (content: {entity_name})
                    }

                    text (content: "Fields:") style (fg: interact)
                    ...field_elements
                }
            }
        }
        Operation::Retrieve {
            entity,
            id,
            select,
            expand,
            ..
        } => {
            let entity_name = entity.set_name();
            let id_str = id.to_string();
            let select_str = if select.is_empty() {
                "All fields".to_string()
            } else {
                select.join(", ")
            };
            let expand_str = if expand.is_empty() {
                "None".to_string()
            } else {
                format!("{} navigation(s)", expand.len())
            };

            element! {
                column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                    text (content: {label.to_string()}) style (bold, fg: primary)

                    row (gap: 1) {
                        text (content: "type") style (fg: muted)
                        text (content: "Retrieve")
                    }
                    row (gap: 1) {
                        text (content: "entity") style (fg: muted)
                        text (content: {entity_name.to_string()})
                    }
                    row (gap: 1) {
                        text (content: "id") style (fg: muted)
                        text (content: {id_str})
                    }
                    row (gap: 1) {
                        text (content: "select") style (fg: muted)
                        text (content: {select_str})
                    }
                    row (gap: 1) {
                        text (content: "expand") style (fg: muted)
                        text (content: {expand_str})
                    }
                }
            }
        }
        Operation::Update {
            entity, id, record, ..
        } => {
            let entity_name = entity.set_name();
            let id_str = id.to_string();
            let mut field_elements = vec![];

            for (key, value) in record.fields().iter() {
                let formatted = format_value(value);
                field_elements.push(element! {
                    row (gap: 1) {
                        text (content: {key.clone()}) style (fg: muted)
                        text (content: {formatted.display})
                    }
                });
            }

            element! {
                column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                    text (content: {label.to_string()}) style (bold, fg: primary)

                    row (gap: 1) {
                        text (content: "type") style (fg: muted)
                        text (content: "Update")
                    }
                    row (gap: 1) {
                        text (content: "entity") style (fg: muted)
                        text (content: {entity_name.to_string()})
                    }
                    row (gap: 1) {
                        text (content: "id") style (fg: muted)
                        text (content: {id_str})
                    }

                    text (content: "Fields:") style (fg: interact)
                    ...field_elements
                }
            }
        }
        Operation::Delete { entity, id, .. } => {
            let entity_name = entity.set_name();
            let id_str = id.to_string();

            element! {
                column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                    text (content: {label.to_string()}) style (bold, fg: primary)

                    row (gap: 1) {
                        text (content: "type") style (fg: muted)
                        text (content: "Delete")
                    }
                    row (gap: 1) {
                        text (content: "entity") style (fg: muted)
                        text (content: {entity_name.to_string()})
                    }
                    row (gap: 1) {
                        text (content: "id") style (fg: muted)
                        text (content: {id_str})
                    }
                }
            }
        }
        Operation::Upsert {
            entity, id, record, ..
        } => {
            let entity_name = entity.set_name();
            let id_str = id.to_string();
            let mut field_elements = vec![];

            for (key, value) in record.fields().iter() {
                let formatted = format_value(value);
                field_elements.push(element! {
                    row (gap: 1) {
                        text (content: {key.clone()}) style (fg: muted)
                        text (content: {formatted.display})
                    }
                });
            }

            element! {
                column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                    text (content: {label.to_string()}) style (bold, fg: primary)

                    row (gap: 1) {
                        text (content: "type") style (fg: muted)
                        text (content: "Upsert")
                    }
                    row (gap: 1) {
                        text (content: "entity") style (fg: muted)
                        text (content: {entity_name.to_string()})
                    }
                    row (gap: 1) {
                        text (content: "id") style (fg: muted)
                        text (content: {id_str})
                    }

                    text (content: "Fields:") style (fg: interact)
                    ...field_elements
                }
            }
        }
        Operation::Associate {
            entity,
            id,
            relationship,
            target_entity,
            target_id,
            ..
        } => {
            let entity_name = entity.set_name();
            let id_str = id.to_string();
            let target_entity_name = target_entity.set_name();
            let target_id_str = target_id.to_string();

            element! {
                column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                    text (content: {label.to_string()}) style (bold, fg: primary)

                    row (gap: 1) {
                        text (content: "type") style (fg: muted)
                        text (content: "Associate")
                    }
                    row (gap: 1) {
                        text (content: "entity") style (fg: muted)
                        text (content: {entity_name.to_string()})
                    }
                    row (gap: 1) {
                        text (content: "id") style (fg: muted)
                        text (content: {id_str})
                    }
                    row (gap: 1) {
                        text (content: "relationship") style (fg: muted)
                        text (content: {relationship.clone()})
                    }
                    row (gap: 1) {
                        text (content: "target_entity") style (fg: muted)
                        text (content: {target_entity_name.to_string()})
                    }
                    row (gap: 1) {
                        text (content: "target_id") style (fg: muted)
                        text (content: {target_id_str})
                    }
                }
            }
        }
        Operation::Disassociate {
            entity,
            id,
            relationship,
            target_id,
            ..
        } => {
            let entity_name = entity.set_name();
            let id_str = id.to_string();
            let target_id_str = target_id.to_string();

            element! {
                column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                    text (content: {label.to_string()}) style (bold, fg: primary)

                    row (gap: 1) {
                        text (content: "type") style (fg: muted)
                        text (content: "Disassociate")
                    }
                    row (gap: 1) {
                        text (content: "entity") style (fg: muted)
                        text (content: {entity_name.to_string()})
                    }
                    row (gap: 1) {
                        text (content: "id") style (fg: muted)
                        text (content: {id_str})
                    }
                    row (gap: 1) {
                        text (content: "relationship") style (fg: muted)
                        text (content: {relationship.clone()})
                    }
                    row (gap: 1) {
                        text (content: "target_id") style (fg: muted)
                        text (content: {target_id_str})
                    }
                }
            }
        }
        Operation::SetLookup {
            entity,
            id,
            nav_property,
            target_entity,
            target_id,
            ..
        } => {
            let entity_name = entity.set_name();
            let id_str = id.to_string();
            let target_entity_name = target_entity.set_name();
            let target_id_str = target_id.to_string();

            element! {
                column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                    text (content: {label.to_string()}) style (bold, fg: primary)

                    row (gap: 1) {
                        text (content: "type") style (fg: muted)
                        text (content: "SetLookup")
                    }
                    row (gap: 1) {
                        text (content: "entity") style (fg: muted)
                        text (content: {entity_name.to_string()})
                    }
                    row (gap: 1) {
                        text (content: "id") style (fg: muted)
                        text (content: {id_str})
                    }
                    row (gap: 1) {
                        text (content: "nav_property") style (fg: muted)
                        text (content: {nav_property.clone()})
                    }
                    row (gap: 1) {
                        text (content: "target_entity") style (fg: muted)
                        text (content: {target_entity_name.to_string()})
                    }
                    row (gap: 1) {
                        text (content: "target_id") style (fg: muted)
                        text (content: {target_id_str})
                    }
                }
            }
        }
        Operation::ClearLookup {
            entity,
            id,
            nav_property,
            ..
        } => {
            let entity_name = entity.set_name();
            let id_str = id.to_string();

            element! {
                column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                    text (content: {label.to_string()}) style (bold, fg: primary)

                    row (gap: 1) {
                        text (content: "type") style (fg: muted)
                        text (content: "ClearLookup")
                    }
                    row (gap: 1) {
                        text (content: "entity") style (fg: muted)
                        text (content: {entity_name.to_string()})
                    }
                    row (gap: 1) {
                        text (content: "id") style (fg: muted)
                        text (content: {id_str})
                    }
                    row (gap: 1) {
                        text (content: "nav_property") style (fg: muted)
                        text (content: {nav_property.clone()})
                    }
                }
            }
        }
    }
}
