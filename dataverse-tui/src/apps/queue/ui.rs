//! Queue app UI rendering helpers.

use std::collections::VecDeque;

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::Text;
use tuidom::Color;

use super::Queue;
use super::repository::StatusCounts;
use super::tree::QueueTreeNode;
use super::types::QueueItem;

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
                        if let QueueTreeNode::Item(item) = &node.value {
                            if item.id == id {
                                return Some(item.clone());
                            }
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
            return Element::col();
        };

        // Find the item from the tree
        let item = self.tree_state.with_ref(|s| {
            // Extract item ID from key
            let item_id: Option<i64> = key
                .strip_prefix("item-")
                .and_then(|rest| rest.split('-').next())
                .and_then(|id_str| id_str.parse().ok());

            item_id.and_then(|id| {
                // Search roots for the matching item
                s.roots.iter().find_map(|node| {
                    if let QueueTreeNode::Item(item) = &node.value {
                        if item.id == id {
                            return Some(item.clone());
                        }
                    }
                    None
                })
            })
        });

        let Some(item) = item else {
            return Element::col();
        };

        let status_text = format!("{:?}", item.status);
        let status_color = Color::var(item.status.color());
        let created = item.created_at.format("%Y-%m-%d %H:%M").to_string();
        let priority_text = item.priority.to_string();
        let ops_text = format!("{} operation(s)", item.payload.operation_count());

        page! {
            column (gap: 1) {
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
