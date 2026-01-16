//! Tree Widget Example
//!
//! Demonstrates the Tree widget with expandable/collapsible nodes.

use std::fs::File;

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{SelectionMode, Text, Tree, TreeItem, TreeNode, TreeState};
use simplelog::{Config, LevelFilter, WriteLogger};
use tuidom::Element;

/// A file system node for the tree.
#[derive(Clone, Debug)]
struct FsNode {
    path: String,
    name: String,
    is_dir: bool,
}

impl TreeItem for FsNode {
    type Key = String;

    fn key(&self) -> String {
        self.path.clone()
    }

    fn render(&self) -> Element {
        let icon = if self.is_dir { "📁" } else { "📄" };
        Element::row()
            .gap(1)
            .child(Element::text(icon))
            .child(Element::text(&self.name))
    }
}

/// Create a sample file system tree.
fn create_sample_tree() -> Vec<TreeNode<FsNode>> {
    vec![
        TreeNode::branch(
            FsNode {
                path: "/home".into(),
                name: "home".into(),
                is_dir: true,
            },
            vec![
                TreeNode::branch(
                    FsNode {
                        path: "/home/user".into(),
                        name: "user".into(),
                        is_dir: true,
                    },
                    vec![
                        TreeNode::branch(
                            FsNode {
                                path: "/home/user/documents".into(),
                                name: "documents".into(),
                                is_dir: true,
                            },
                            vec![
                                TreeNode::leaf(FsNode {
                                    path: "/home/user/documents/report.pdf".into(),
                                    name: "report.pdf".into(),
                                    is_dir: false,
                                }),
                                TreeNode::leaf(FsNode {
                                    path: "/home/user/documents/notes.txt".into(),
                                    name: "notes.txt".into(),
                                    is_dir: false,
                                }),
                                TreeNode::branch(
                                    FsNode {
                                        path: "/home/user/documents/work".into(),
                                        name: "work".into(),
                                        is_dir: true,
                                    },
                                    vec![
                                        TreeNode::leaf(FsNode {
                                            path: "/home/user/documents/work/project.md".into(),
                                            name: "project.md".into(),
                                            is_dir: false,
                                        }),
                                        TreeNode::leaf(FsNode {
                                            path: "/home/user/documents/work/tasks.md".into(),
                                            name: "tasks.md".into(),
                                            is_dir: false,
                                        }),
                                    ],
                                ),
                            ],
                        ),
                        TreeNode::branch(
                            FsNode {
                                path: "/home/user/downloads".into(),
                                name: "downloads".into(),
                                is_dir: true,
                            },
                            vec![
                                TreeNode::leaf(FsNode {
                                    path: "/home/user/downloads/image.png".into(),
                                    name: "image.png".into(),
                                    is_dir: false,
                                }),
                                TreeNode::leaf(FsNode {
                                    path: "/home/user/downloads/archive.zip".into(),
                                    name: "archive.zip".into(),
                                    is_dir: false,
                                }),
                            ],
                        ),
                        TreeNode::leaf(FsNode {
                            path: "/home/user/.bashrc".into(),
                            name: ".bashrc".into(),
                            is_dir: false,
                        }),
                    ],
                ),
            ],
        ),
        TreeNode::branch(
            FsNode {
                path: "/etc".into(),
                name: "etc".into(),
                is_dir: true,
            },
            vec![
                TreeNode::leaf(FsNode {
                    path: "/etc/hosts".into(),
                    name: "hosts".into(),
                    is_dir: false,
                }),
                TreeNode::leaf(FsNode {
                    path: "/etc/passwd".into(),
                    name: "passwd".into(),
                    is_dir: false,
                }),
                TreeNode::branch(
                    FsNode {
                        path: "/etc/nginx".into(),
                        name: "nginx".into(),
                        is_dir: true,
                    },
                    vec![
                        TreeNode::leaf(FsNode {
                            path: "/etc/nginx/nginx.conf".into(),
                            name: "nginx.conf".into(),
                            is_dir: false,
                        }),
                    ],
                ),
            ],
        ),
        TreeNode::branch(
            FsNode {
                path: "/var".into(),
                name: "var".into(),
                is_dir: true,
            },
            vec![
                TreeNode::branch(
                    FsNode {
                        path: "/var/log".into(),
                        name: "log".into(),
                        is_dir: true,
                    },
                    vec![
                        TreeNode::leaf(FsNode {
                            path: "/var/log/syslog".into(),
                            name: "syslog".into(),
                            is_dir: false,
                        }),
                        TreeNode::leaf(FsNode {
                            path: "/var/log/auth.log".into(),
                            name: "auth.log".into(),
                            is_dir: false,
                        }),
                    ],
                ),
            ],
        ),
        // Add many more items to test scrolling
        TreeNode::branch(
            FsNode {
                path: "/usr".into(),
                name: "usr".into(),
                is_dir: true,
            },
            vec![
                TreeNode::branch(
                    FsNode {
                        path: "/usr/bin".into(),
                        name: "bin".into(),
                        is_dir: true,
                    },
                    (1..=30)
                        .map(|i| {
                            TreeNode::leaf(FsNode {
                                path: format!("/usr/bin/program{}", i),
                                name: format!("program{}", i),
                                is_dir: false,
                            })
                        })
                        .collect(),
                ),
                TreeNode::branch(
                    FsNode {
                        path: "/usr/lib".into(),
                        name: "lib".into(),
                        is_dir: true,
                    },
                    (1..=20)
                        .map(|i| {
                            TreeNode::leaf(FsNode {
                                path: format!("/usr/lib/lib{}.so", i),
                                name: format!("lib{}.so", i),
                                is_dir: false,
                            })
                        })
                        .collect(),
                ),
                TreeNode::branch(
                    FsNode {
                        path: "/usr/share".into(),
                        name: "share".into(),
                        is_dir: true,
                    },
                    (1..=15)
                        .map(|i| {
                            TreeNode::leaf(FsNode {
                                path: format!("/usr/share/doc{}", i),
                                name: format!("doc{}", i),
                                is_dir: false,
                            })
                        })
                        .collect(),
                ),
            ],
        ),
        TreeNode::branch(
            FsNode {
                path: "/tmp".into(),
                name: "tmp".into(),
                is_dir: true,
            },
            (1..=25)
                .map(|i| {
                    TreeNode::leaf(FsNode {
                        path: format!("/tmp/temp_file_{}", i),
                        name: format!("temp_file_{}", i),
                        is_dir: false,
                    })
                })
                .collect(),
        ),
    ]
}

#[app]
struct TreeExample {
    files: TreeState<FsNode>,
    message: String,
}

#[app_impl]
impl TreeExample {
    #[on_start]
    async fn on_start(&self) {
        let tree = create_sample_tree();
        self.files
            .set(TreeState::new(tree).with_selection(SelectionMode::Single).with_roots_expanded());
        self.message
            .set("Navigate with arrows, Enter to select".into());
    }

    #[keybinds]
    fn keys() {
        bind("q", quit);
    }

    #[handler]
    async fn quit(&self, gx: &GlobalContext) {
        gx.shutdown();
    }

    #[handler]
    async fn node_selected(&self) {
        let state = self.files.get();
        if let Some(key) = &state.last_activated {
            self.message.set(format!("Selected: {}", key));
        }
    }

    #[handler]
    async fn node_activated(&self, gx: &GlobalContext) {
        let state = self.files.get();
        if let Some(key) = &state.last_activated {
            // Find if it's a directory
            if let Some(node) = state.find_node(key) {
                if node.value.is_dir {
                    gx.toast(Toast::info(format!("Selected directory: {}", node.value.name)));
                } else {
                    gx.toast(Toast::info(format!("Opened file: {}", node.value.name)));
                }
            }
        }
    }

    fn element(&self) -> Element {
        let message = self.message.get();

        page! {
            column (padding: 2, gap: 1, height: fill, width: fill) style (bg: background) {
                // Header
                text (content: "Tree Widget Demo") style (bold, fg: accent)
                text (content: "Use arrows to navigate, Left/Right to collapse/expand") style (fg: muted)

                // Status
                row (gap: 1) {
                    text (content: "Status:") style (fg: muted)
                    text (content: {message}) style (fg: accent)
                }

                // Tree view
                box_ (id: "tree-container", height: fill, width: fill) style (bg: surface) {
                    tree (state: self.files, id: "file-tree")
                        on_select: node_selected()
                        on_activate: node_activated()
                }

                // Footer
                row (gap: 2) {
                    text (content: "Press 'q' to quit") style (fg: muted)
                    text (content: "| Left: collapse/parent | Right: expand/child") style (fg: muted)
                }
            }
        }
    }
}

#[tokio::main]
async fn main() {
    // Set up file logging
    let log_file = File::create("tree.log").expect("Failed to create log file");
    WriteLogger::init(LevelFilter::Debug, Config::default(), log_file)
        .expect("Failed to initialize logger");

    if let Err(e) = Runtime::new()
        .expect("Failed to create runtime")
        .run(TreeExample::default())
        .await
    {
        eprintln!("Error: {}", e);
    }
}
