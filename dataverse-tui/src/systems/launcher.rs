use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{SelectionMode, Text, Tree, TreeItem, TreeNode, TreeState};
use tuidom::Element;

use crate::TestApp;

/// A launcher entry (category or app).
#[derive(Clone, Debug)]
pub struct LauncherEntry {
    id: String,
    name: String,
    is_category: bool,
}

impl TreeItem for LauncherEntry {
    type Key = String;

    fn key(&self) -> String {
        self.id.clone()
    }

    fn render(&self) -> Element {
        Element::text(&self.name)
    }
}

fn category(id: &str, name: &str) -> LauncherEntry {
    LauncherEntry {
        id: id.into(),
        name: name.into(),
        is_category: true,
    }
}

fn app(id: &str, name: &str) -> LauncherEntry {
    LauncherEntry {
        id: id.into(),
        name: name.into(),
        is_category: false,
    }
}

fn create_launcher_tree() -> Vec<TreeNode<LauncherEntry>> {
    vec![
        TreeNode::branch(
            category("data", "Data"),
            vec![
                TreeNode::leaf(app("record-viewer", "Record Viewer")),
                TreeNode::leaf(app("entity-viewer", "Entity Viewer")),
                TreeNode::leaf(app("collection-browser", "Collection Browser")),
                TreeNode::leaf(app("search", "Search")),
                TreeNode::leaf(app("relationships", "Relationships")),
                TreeNode::leaf(app("query-builder", "Query Builder")),
            ],
        ),
        TreeNode::branch(
            category("transfer", "Transfer"),
            vec![
                TreeNode::leaf(app("import", "Import")),
                TreeNode::leaf(app("export", "Export")),
                TreeNode::leaf(app("queue", "Queue")),
                TreeNode::leaf(app("transform", "Transform")),
            ],
        ),
        TreeNode::branch(
            category("system", "System"),
            vec![
                TreeNode::leaf(app("indexer", "Indexer")),
                TreeNode::leaf(app("cache", "Cache")),
                TreeNode::leaf(app("settings", "Settings")),
                TreeNode::leaf(app("connections", "Connections")),
                TreeNode::leaf(app("logs", "Logs")),
                TreeNode::leaf(app("test", "Test")),
            ],
        ),
        TreeNode::branch(
            category("custom", "Custom"),
            vec![],
        ),
    ]
}

#[modal(size = Lg)]
struct LauncherModal {
    entries: TreeState<LauncherEntry>,
}

#[modal_impl]
impl LauncherModal {
    async fn on_start(&self) {
        let tree = create_launcher_tree();
        self.entries.set(
            TreeState::new(tree)
                .with_selection(SelectionMode::None)
                .with_roots_expanded(),
        );
    }

    #[keybinds]
    fn keys() {
        bind("escape", close);
    }

    #[handler]
    async fn close(&self, mx: &ModalContext<Option<String>>) {
        mx.close(None);
    }

    #[handler]
    async fn on_activate(&self, mx: &ModalContext<Option<String>>) {
        let state = self.entries.get();
        if let Some(key) = &state.last_activated {
            // Only close for apps, not categories
            if let Some(node) = state.find_node(key) {
                if !node.value.is_category {
                    mx.close(Some(key.clone()));
                }
            }
        }
    }

    fn element(&self) -> Element {
        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: background) {
                text (content: "Launcher") style (bold, fg: accent)

                box_ (id: "tree-container", height: fill, width: fill) style (bg: surface) {
                    tree (state: self.entries, id: "launcher-tree")
                        on_activate: on_activate()
                }

                row (width: fill, justify: between) {
                    row (gap: 1) {
                        text (content: "esc") style (fg: primary)
                        text (content: "close") style (fg: muted)
                    }
                    row (gap: 1) {
                        text (content: "enter") style (fg: primary)
                        text (content: "select") style (fg: muted)
                    }
                }
            }
        }
    }
}

#[system]
pub struct Launcher;

#[system_impl]
impl Launcher {
    #[keybinds]
    fn keys() {
        bind("ctrl+p", open_launcher);
    }

    #[handler]
    async fn open_launcher(&self, gx: &GlobalContext) {
        let result = gx.modal(LauncherModal::default()).await;

        if let Some(selected) = result {
            match selected.as_str() {
                "test" => {
                    let _ = gx.spawn_and_focus(TestApp::default());
                }
                _ => {
                    gx.toast(Toast::info(format!("App not implemented: {}", selected)));
                }
            }
        }
    }
}
