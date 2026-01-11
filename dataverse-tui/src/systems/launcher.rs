use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{SelectionMode, Text, Tree, TreeItem, TreeNode, TreeState};
use tuidom::Element;

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
        let icon = if self.is_category { ">" } else { "-" };
        Element::row()
            .gap(1)
            .child(Element::text(icon))
            .child(Element::text(&self.name))
    }
}

fn create_launcher_tree() -> Vec<TreeNode<LauncherEntry>> {
    vec![TreeNode::branch(
        LauncherEntry {
            id: "placeholder".into(),
            name: "Placeholder".into(),
            is_category: true,
        },
        vec![],
    )]
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
                .with_selection(SelectionMode::Single)
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
            mx.close(Some(key.clone()));
        }
    }

    fn element(&self) -> Element {
        page! {
            column (padding: 2, gap: 1, width: fill, height: fill) style (bg: background) {
                text (content: "Launcher") style (bold, fg: primary)
                text (content: "Select an app to launch") style (fg: muted)

                box_ (id: "tree-container", height: fill, width: fill) style (bg: surface) {
                    tree (state: self.entries, id: "launcher-tree")
                        on_activate: on_activate()
                }

                text (content: "Press Escape to close") style (fg: muted)
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
            gx.toast(Toast::info(format!("Selected: {}", selected)));
        }
    }
}
