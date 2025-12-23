//! Tree Example
//!
//! Demonstrates the Tree widget:
//! - Hierarchical data with expand/collapse
//! - Keyboard navigation (arrows, Enter, Space)
//! - Selection modes (single, multi)
//! - Event handlers for node actions

use std::fs::File;

use log::LevelFilter;
use rafter::prelude::*;
use simplelog::{Config, WriteLogger};

// =============================================================================
// File tree item
// =============================================================================

#[derive(Clone, Debug)]
struct FileNode {
    /// Unique path (used as ID)
    path: String,
    /// Display name
    name: String,
    /// Is this a directory?
    is_dir: bool,
    /// Child nodes (only for directories)
    children: Vec<FileNode>,
}

impl FileNode {
    fn file(path: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            name: name.into(),
            is_dir: false,
            children: vec![],
        }
    }

    fn dir(path: impl Into<String>, name: impl Into<String>, children: Vec<FileNode>) -> Self {
        Self {
            path: path.into(),
            name: name.into(),
            is_dir: true,
            children,
        }
    }
}

impl TreeItem for FileNode {
    fn id(&self) -> String {
        self.path.clone()
    }

    fn children(&self) -> Vec<Self> {
        self.children.clone()
    }

    fn render(&self, focused: bool, selected: bool, depth: u16, expanded: bool) -> Node {
        // Use default styling helpers - simple and consistent!
        Self::render_default(&self.name, focused, selected, depth, self.is_dir, expanded)
    }
}

// =============================================================================
// App state
// =============================================================================

#[app]
struct TreeDemo {
    tree: Tree<FileNode>,
    status: String,
}

#[app_impl]
impl TreeDemo {
    async fn on_start(&self, _cx: &AppContext) {
        // Create a large sample file tree to test scrolling
        let tree_data = vec![
            // Main source directory with many subdirectories
            FileNode::dir(
                "/src",
                "src",
                vec![
                    FileNode::file("/src/main.rs", "main.rs"),
                    FileNode::file("/src/lib.rs", "lib.rs"),
                    FileNode::file("/src/config.rs", "config.rs"),
                    FileNode::file("/src/error.rs", "error.rs"),
                    FileNode::file("/src/prelude.rs", "prelude.rs"),
                    // Components directory
                    FileNode::dir(
                        "/src/widgets",
                        "widgets",
                        vec![
                            FileNode::file("/src/widgets/mod.rs", "mod.rs"),
                            FileNode::dir(
                                "/src/widgets/button",
                                "button",
                                vec![
                                    FileNode::file("/src/widgets/button/mod.rs", "mod.rs"),
                                    FileNode::file("/src/widgets/button/state.rs", "state.rs"),
                                    FileNode::file("/src/widgets/button/render.rs", "render.rs"),
                                    FileNode::file("/src/widgets/button/events.rs", "events.rs"),
                                ],
                            ),
                            FileNode::dir(
                                "/src/widgets/input",
                                "input",
                                vec![
                                    FileNode::file("/src/widgets/input/mod.rs", "mod.rs"),
                                    FileNode::file("/src/widgets/input/state.rs", "state.rs"),
                                    FileNode::file("/src/widgets/input/render.rs", "render.rs"),
                                    FileNode::file("/src/widgets/input/events.rs", "events.rs"),
                                    FileNode::file(
                                        "/src/widgets/input/validation.rs",
                                        "validation.rs",
                                    ),
                                ],
                            ),
                            FileNode::dir(
                                "/src/widgets/list",
                                "list",
                                vec![
                                    FileNode::file("/src/widgets/list/mod.rs", "mod.rs"),
                                    FileNode::file("/src/widgets/list/state.rs", "state.rs"),
                                    FileNode::file("/src/widgets/list/render.rs", "render.rs"),
                                    FileNode::file("/src/widgets/list/events.rs", "events.rs"),
                                    FileNode::file("/src/widgets/list/item.rs", "item.rs"),
                                    FileNode::file("/src/widgets/list/any_list.rs", "any_list.rs"),
                                ],
                            ),
                            FileNode::dir(
                                "/src/widgets/tree",
                                "tree",
                                vec![
                                    FileNode::file("/src/widgets/tree/mod.rs", "mod.rs"),
                                    FileNode::file("/src/widgets/tree/state.rs", "state.rs"),
                                    FileNode::file("/src/widgets/tree/render.rs", "render.rs"),
                                    FileNode::file("/src/widgets/tree/events.rs", "events.rs"),
                                    FileNode::file("/src/widgets/tree/item.rs", "item.rs"),
                                    FileNode::file("/src/widgets/tree/any_tree.rs", "any_tree.rs"),
                                ],
                            ),
                            FileNode::dir(
                                "/src/widgets/scroll_area",
                                "scroll_area",
                                vec![
                                    FileNode::file("/src/widgets/scroll_area/mod.rs", "mod.rs"),
                                    FileNode::file("/src/widgets/scroll_area/state.rs", "state.rs"),
                                    FileNode::file(
                                        "/src/widgets/scroll_area/render.rs",
                                        "render.rs",
                                    ),
                                    FileNode::file(
                                        "/src/widgets/scroll_area/events.rs",
                                        "events.rs",
                                    ),
                                ],
                            ),
                            FileNode::dir(
                                "/src/widgets/scrollbar",
                                "scrollbar",
                                vec![
                                    FileNode::file("/src/widgets/scrollbar/mod.rs", "mod.rs"),
                                    FileNode::file("/src/widgets/scrollbar/state.rs", "state.rs"),
                                    FileNode::file("/src/widgets/scrollbar/render.rs", "render.rs"),
                                    FileNode::file("/src/widgets/scrollbar/types.rs", "types.rs"),
                                ],
                            ),
                        ],
                    ),
                    // Runtime directory
                    FileNode::dir(
                        "/src/runtime",
                        "runtime",
                        vec![
                            FileNode::file("/src/runtime/mod.rs", "mod.rs"),
                            FileNode::file("/src/runtime/event_loop.rs", "event_loop.rs"),
                            FileNode::file("/src/runtime/events.rs", "events.rs"),
                            FileNode::file("/src/runtime/terminal.rs", "terminal.rs"),
                            FileNode::file("/src/runtime/input.rs", "input.rs"),
                            FileNode::file("/src/runtime/hit_test.rs", "hit_test.rs"),
                            FileNode::file("/src/runtime/modal.rs", "modal.rs"),
                            FileNode::dir(
                                "/src/runtime/render",
                                "render",
                                vec![
                                    FileNode::file("/src/runtime/render/mod.rs", "mod.rs"),
                                    FileNode::file("/src/runtime/render/layout.rs", "layout.rs"),
                                    FileNode::file(
                                        "/src/runtime/render/backdrop.rs",
                                        "backdrop.rs",
                                    ),
                                    FileNode::file("/src/runtime/render/toasts.rs", "toasts.rs"),
                                    FileNode::dir(
                                        "/src/runtime/render/primitives",
                                        "primitives",
                                        vec![
                                            FileNode::file(
                                                "/src/runtime/render/primitives/mod.rs",
                                                "mod.rs",
                                            ),
                                            FileNode::file(
                                                "/src/runtime/render/primitives/text.rs",
                                                "text.rs",
                                            ),
                                            FileNode::file(
                                                "/src/runtime/render/primitives/button.rs",
                                                "button.rs",
                                            ),
                                            FileNode::file(
                                                "/src/runtime/render/primitives/container.rs",
                                                "container.rs",
                                            ),
                                        ],
                                    ),
                                ],
                            ),
                        ],
                    ),
                    // Utils directory
                    FileNode::dir(
                        "/src/utils",
                        "utils",
                        vec![
                            FileNode::file("/src/utils/mod.rs", "mod.rs"),
                            FileNode::file("/src/utils/text.rs", "text.rs"),
                            FileNode::file("/src/utils/geometry.rs", "geometry.rs"),
                            FileNode::file("/src/utils/colors.rs", "colors.rs"),
                        ],
                    ),
                    // Node directory
                    FileNode::dir(
                        "/src/node",
                        "node",
                        vec![
                            FileNode::file("/src/node/mod.rs", "mod.rs"),
                            FileNode::file("/src/node/layout.rs", "layout.rs"),
                        ],
                    ),
                ],
            ),
            // Tests directory
            FileNode::dir(
                "/tests",
                "tests",
                vec![
                    FileNode::file("/tests/integration.rs", "integration.rs"),
                    FileNode::file("/tests/unit.rs", "unit.rs"),
                    FileNode::file("/tests/geometry.rs", "geometry.rs"),
                    FileNode::file("/tests/text.rs", "text.rs"),
                    FileNode::file("/tests/theme.rs", "theme.rs"),
                    FileNode::dir(
                        "/tests/widgets",
                        "widgets",
                        vec![
                            FileNode::file("/tests/widgets/list.rs", "list.rs"),
                            FileNode::file("/tests/widgets/tree.rs", "tree.rs"),
                            FileNode::file("/tests/widgets/input.rs", "input.rs"),
                        ],
                    ),
                ],
            ),
            // Examples directory
            FileNode::dir(
                "/examples",
                "examples",
                vec![
                    FileNode::file("/examples/counter.rs", "counter.rs"),
                    FileNode::file("/examples/pagination.rs", "pagination.rs"),
                    FileNode::file("/examples/reader.rs", "reader.rs"),
                    FileNode::file("/examples/tree.rs", "tree.rs"),
                    FileNode::dir(
                        "/examples/explorer",
                        "explorer",
                        vec![
                            FileNode::file("/examples/explorer/main.rs", "main.rs"),
                            FileNode::dir(
                                "/examples/explorer/pages",
                                "pages",
                                vec![
                                    FileNode::file("/examples/explorer/pages/mod.rs", "mod.rs"),
                                    FileNode::file("/examples/explorer/pages/list.rs", "list.rs"),
                                    FileNode::file(
                                        "/examples/explorer/pages/detail.rs",
                                        "detail.rs",
                                    ),
                                ],
                            ),
                            FileNode::dir(
                                "/examples/explorer/modals",
                                "modals",
                                vec![
                                    FileNode::file("/examples/explorer/modals/mod.rs", "mod.rs"),
                                    FileNode::file(
                                        "/examples/explorer/modals/confirm.rs",
                                        "confirm.rs",
                                    ),
                                    FileNode::file(
                                        "/examples/explorer/modals/rename.rs",
                                        "rename.rs",
                                    ),
                                ],
                            ),
                        ],
                    ),
                ],
            ),
            // Docs directory
            FileNode::dir(
                "/docs",
                "docs",
                vec![
                    FileNode::file("/docs/overview.md", "overview.md"),
                    FileNode::file("/docs/architecture.md", "architecture.md"),
                    FileNode::file("/docs/pages.md", "pages.md"),
                    FileNode::file("/docs/state.md", "state.md"),
                    FileNode::file("/docs/styling.md", "styling.md"),
                    FileNode::file("/docs/interactions.md", "interactions.md"),
                    FileNode::file("/docs/errors.md", "errors.md"),
                    FileNode::file("/docs/apps.md", "apps.md"),
                    FileNode::file("/docs/overlays.md", "overlays.md"),
                    FileNode::file("/docs/async.md", "async.md"),
                    FileNode::file("/docs/animations.md", "animations.md"),
                ],
            ),
            // Assets directory with many icons
            FileNode::dir(
                "/assets",
                "assets",
                vec![
                    FileNode::dir(
                        "/assets/icons",
                        "icons",
                        (1..=20)
                            .map(|i| {
                                FileNode::file(
                                    format!("/assets/icons/icon_{}.svg", i),
                                    format!("icon_{}.svg", i),
                                )
                            })
                            .collect(),
                    ),
                    FileNode::dir(
                        "/assets/themes",
                        "themes",
                        vec![
                            FileNode::file("/assets/themes/dark.toml", "dark.toml"),
                            FileNode::file("/assets/themes/light.toml", "light.toml"),
                            FileNode::file("/assets/themes/solarized.toml", "solarized.toml"),
                            FileNode::file("/assets/themes/monokai.toml", "monokai.toml"),
                            FileNode::file("/assets/themes/nord.toml", "nord.toml"),
                        ],
                    ),
                ],
            ),
            // Scripts directory
            FileNode::dir(
                "/scripts",
                "scripts",
                vec![
                    FileNode::file("/scripts/build.sh", "build.sh"),
                    FileNode::file("/scripts/test.sh", "test.sh"),
                    FileNode::file("/scripts/release.sh", "release.sh"),
                    FileNode::file("/scripts/benchmark.sh", "benchmark.sh"),
                ],
            ),
            // Config files at root
            FileNode::file("/Cargo.toml", "Cargo.toml"),
            FileNode::file("/Cargo.lock", "Cargo.lock"),
            FileNode::file("/README.md", "README.md"),
            FileNode::file("/LICENSE", "LICENSE"),
            FileNode::file("/.gitignore", ".gitignore"),
            FileNode::file("/.rustfmt.toml", ".rustfmt.toml"),
            FileNode::file("/clippy.toml", "clippy.toml"),
            FileNode::file("/rust-toolchain.toml", "rust-toolchain.toml"),
        ];

        self.tree.set_items(tree_data);
        self.tree.set_selection_mode(SelectionMode::Multiple);
        // Expand a few directories by default to show some depth
        self.tree.expand("/src");
        self.tree.expand("/src/widgets");
        self.status.set(
            "Ready. Use arrows to navigate, Enter to expand/collapse, Space to select.".to_string(),
        );
    }

    #[keybinds]
    fn keys() -> Keybinds {
        keybinds! {
            "q" => quit,
            "e" => expand_all,
            "c" => collapse_all,
        }
    }

    #[handler]
    async fn quit(&self, cx: &AppContext) {
        cx.exit();
    }

    #[handler]
    async fn expand_all(&self, _cx: &AppContext) {
        self.tree.expand_all();
        self.status.set("Expanded all nodes".to_string());
    }

    #[handler]
    async fn collapse_all(&self, _cx: &AppContext) {
        self.tree.collapse_all();
        self.status.set("Collapsed all nodes".to_string());
    }

    #[handler]
    async fn on_activate(&self, cx: &AppContext) {
        if let Some(id) = cx.activated_id() {
            if let Some(node) = self.tree.find(&id) {
                if node.is_dir {
                    // Toggle expand/collapse for directories
                    self.tree.toggle(&id);
                    let action = if self.tree.is_expanded(&id) {
                        "Expanded"
                    } else {
                        "Collapsed"
                    };
                    self.status.set(format!("{}: {}", action, node.name));
                } else {
                    // "Open" file
                    self.status.set(format!("Opened: {}", node.path));
                }
            }
        }
    }

    #[handler]
    async fn on_expand(&self, cx: &AppContext) {
        if let Some(id) = cx.expanded_id() {
            if let Some(node) = self.tree.find(&id) {
                self.status.set(format!("Expanded: {}", node.name));
            }
        }
    }

    #[handler]
    async fn on_collapse(&self, cx: &AppContext) {
        if let Some(id) = cx.collapsed_id() {
            if let Some(node) = self.tree.find(&id) {
                self.status.set(format!("Collapsed: {}", node.name));
            }
        }
    }

    #[handler]
    async fn on_selection(&self, cx: &AppContext) {
        if let Some(ids) = cx.selected_ids() {
            let count = ids.len();
            if count == 0 {
                self.status.set("Selection cleared".to_string());
            } else if count == 1 {
                self.status.set(format!("Selected: {}", ids[0]));
            } else {
                self.status.set(format!("Selected {} items", count));
            }
        }
    }

    #[handler]
    async fn on_cursor(&self, cx: &AppContext) {
        if let Some(id) = cx.cursor_id() {
            if let Some(node) = self.tree.find(&id) {
                let kind = if node.is_dir { "dir" } else { "file" };
                self.status.set(format!("Cursor: {} ({})", node.name, kind));
            }
        }
    }

    fn page(&self) -> Node {
        let status = self.status.get();
        let selected_count = self.tree.selected_ids().len();
        let visible_count = self.tree.visible_len();

        page! {
            column (padding: 1, gap: 1, bg: background) {
                // Header
                column {
                    text (bold, fg: primary) { "Tree Demo" }
                    text (fg: muted) { "File explorer example" }
                }

                // Tree container
                column (border: rounded, height: fill, width: fill) {
                    tree (
                        bind: self.tree,
                        on_activate: on_activate,
                        on_expand: on_expand,
                        on_collapse: on_collapse,
                        on_selection_change: on_selection,
                        on_cursor_move: on_cursor,
                        height: fill,
                        width: fill
                    )
                }

                // Status bar
                row (gap: 2) {
                    text (fg: muted) { format!("{} visible", visible_count) }
                    text (fg: muted) { "|" }
                    text (fg: muted) { format!("{} selected", selected_count) }
                    text (fg: muted) { "|" }
                    text (fg: text) { status }
                }

                // Help
                text (fg: muted) { "Arrows: navigate  Enter: open  Space: select  e: expand all  c: collapse all  q: quit" }
            }
        }
    }
}

#[tokio::main]
async fn main() {
    // Initialize file logging
    if let Ok(log_file) = File::create("tree.log") {
        let _ = WriteLogger::init(LevelFilter::Debug, Config::default(), log_file);
    }

    if let Err(e) = rafter::Runtime::new().start_with::<TreeDemo>().await {
        eprintln!("Error: {}", e);
    }
}
