//! Query Builder app for constructing OData queries visually.

pub mod data;
mod tree;

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{Text, Tree, TreeState};

use data::QueryData;
use tree::{QueryTreeNode, build_tree};

/// Query Builder app: visual tree-based OData query construction.
#[app(name = "Query Builder")]
pub struct QueryBuilder {
    /// Tree widget state.
    tree_state: TreeState<QueryTreeNode>,
    /// The query being constructed.
    query: QueryData,
}

#[app_impl]
impl QueryBuilder {
    #[on_start]
    async fn on_start(&self) {
        self.rebuild_tree();
        self.tree_state.update(|s| {
            s.expand(&"section-Entity".to_string());
            s.expand(&"section-Select".to_string());
            s.expand(&"section-Filter".to_string());
            s.expand(&"section-OrderBy".to_string());
            s.expand(&"section-Top".to_string());
        });
    }

    fn title(&self) -> String {
        self.query.with_ref(|q| match &q.entity {
            Some(entity) => format!("Query Builder ({})", entity),
            None => "Query Builder".to_string(),
        })
    }

    // =========================================================================
    // Keybinds
    // =========================================================================

    #[keybinds]
    fn keybinds() {
        bind("escape", close_app);
        bind("t", toggle_group);
        bind("d", delete_node);
    }

    #[handler]
    async fn close_app(&self, gx: &GlobalContext, cx: &AppContext) {
        let confirmed = gx
            .modal(crate::modals::ConfirmModal::new("Close the query builder?"))
            .await;
        if confirmed {
            cx.close();
        }
    }

    /// Toggle AND/OR on the focused filter group.
    #[handler]
    async fn toggle_group(&self) {
        let Some(key) = self.focused_key() else {
            return;
        };

        // Only works on filter group nodes
        let Some(group_id) = key.strip_prefix("filter-group-") else {
            return;
        };
        let Ok(id) = group_id.parse::<usize>() else {
            return;
        };

        self.query.update(|q| q.filter.toggle_group(id));
        self.rebuild_tree();
    }

    /// Delete the focused node (select field, filter condition/group, sort item, top value).
    #[handler]
    async fn delete_node(&self, gx: &GlobalContext) {
        let Some(key) = self.focused_key() else {
            return;
        };

        if let Some(idx_str) = key.strip_prefix("select-") {
            if let Ok(idx) = idx_str.parse::<usize>() {
                let len = self.query.with_ref(|q| q.select.len());
                if idx < len {
                    self.query.update(|q| {
                        q.select.remove(idx);
                    });
                    self.rebuild_tree();
                }
            }
        } else if let Some(id_str) = key.strip_prefix("filter-cond-") {
            if let Ok(id) = id_str.parse::<usize>() {
                self.query.update(|q| {
                    q.filter.remove_node(id);
                });
                self.rebuild_tree();
            }
        } else if let Some(id_str) = key.strip_prefix("filter-group-") {
            if let Ok(id) = id_str.parse::<usize>() {
                let has_children = self.query.with_ref(|q| q.filter.group_has_children(id));
                if has_children {
                    let confirmed = gx
                        .modal(crate::modals::ConfirmModal::new(
                            "Delete this group and all its children?",
                        ))
                        .await;
                    if !confirmed {
                        return;
                    }
                }
                self.query.update(|q| {
                    q.filter.remove_node(id);
                });
                self.rebuild_tree();
            }
        } else if let Some(id_str) = key.strip_prefix("sort-") {
            if let Ok(id) = id_str.parse::<usize>() {
                self.query.update(|q| {
                    q.order_by.retain(|sf| sf.id != id);
                });
                self.rebuild_tree();
            }
        } else if key == "top-value" {
            self.query.update(|q| {
                q.top = None;
            });
            self.rebuild_tree();
        }
    }

    /// Tree node activated (Enter/Space).
    #[handler]
    async fn on_activate(&self, gx: &GlobalContext) {
        let Some(key) = self.focused_key() else {
            return;
        };

        // TODO: Open appropriate modal based on focused node type
        match key.as_str() {
            "section-Entity" | "entity-value" => {
                gx.toast(Toast::info("Entity picker not yet implemented"));
            }
            k if k == "section-Select" || k.starts_with("select-") => {
                gx.toast(Toast::info("Field picker not yet implemented"));
            }
            k if k == "section-Top" || k == "top-value" => {
                gx.toast(Toast::info("Number editor not yet implemented"));
            }
            k if k == "section-OrderBy" || k.starts_with("sort-") => {
                gx.toast(Toast::info("Sort editor not yet implemented"));
            }
            k if k.starts_with("filter-cond-") => {
                gx.toast(Toast::info("Condition editor not yet implemented"));
            }
            _ => {}
        }
    }

    // =========================================================================
    // Internal
    // =========================================================================

    /// Get the key of the currently focused tree node.
    fn focused_key(&self) -> Option<String> {
        self.tree_state.with_ref(|s| s.focused_key.clone())
    }

    /// Rebuild the tree widget state from the current QueryData.
    fn rebuild_tree(&self) {
        let nodes = self.query.with_ref(|q| build_tree(q));
        self.tree_state.update(|s| {
            s.set_roots(nodes);
        });
    }

    // =========================================================================
    // UI
    // =========================================================================

    fn element(&self) -> Element {
        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: background) {
                // Header
                row (width: fill, justify: between) {
                    text (content: "Query Builder") style (bold, fg: interact)
                    row (gap: 2) {
                        row (gap: 1) {
                            text (content: "a") style (fg: primary)
                            text (content: "add") style (fg: muted)
                        }
                        row (gap: 1) {
                            text (content: "d") style (fg: primary)
                            text (content: "delete") style (fg: muted)
                        }
                        row (gap: 1) {
                            text (content: "t") style (fg: primary)
                            text (content: "toggle") style (fg: muted)
                        }
                    }
                }

                // Tree
                box_ (id: "query-tree-container", height: fill, width: fill) style (bg: surface) {
                    tree (state: self.tree_state, id: "query-tree")
                        on_activate: on_activate()
                }

                // Footer
                row (width: fill) {
                    row (gap: 1) {
                        text (content: "esc") style (fg: primary)
                        text (content: "close") style (fg: muted)
                    }
                }
            }
        }
    }
}
