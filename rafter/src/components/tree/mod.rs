//! Tree component for displaying hierarchical data.
//!
//! The Tree component provides a virtualized, expandable/collapsible tree view
//! with selection support.
//!
//! # Example
//!
//! ```ignore
//! use rafter::prelude::*;
//!
//! #[derive(Clone, Debug)]
//! struct FileNode {
//!     path: String,
//!     name: String,
//!     is_dir: bool,
//!     children: Vec<FileNode>,
//! }
//!
//! impl TreeItem for FileNode {
//!     fn id(&self) -> String {
//!         self.path.clone()
//!     }
//!
//!     fn children(&self) -> Vec<Self> {
//!         self.children.clone()
//!     }
//!
//!     fn render(&self, focused: bool, selected: bool, depth: u16, expanded: bool) -> Node {
//!         let indent = "  ".repeat(depth as usize);
//!         let icon = if self.is_dir {
//!             if expanded { "▼ " } else { "▶ " }
//!         } else {
//!             "  "
//!         };
//!         
//!         let fg = if focused { "primary" } else if selected { "secondary" } else { "text" };
//!         
//!         view! {
//!             text (fg: fg) { format!("{}{}{}", indent, icon, self.name) }
//!         }
//!     }
//! }
//!
//! // In your app:
//! let tree = Tree::with_items(vec![root_node]);
//! tree.expand("/home/user");
//!
//! // In your view:
//! view! {
//!     tree (bind: self.file_tree, on_activate: open_file, on_expand: load_children)
//! }
//! ```

mod any_tree;
mod events;
mod item;
mod state;

pub use any_tree::AnyTree;
pub use item::TreeItem;
pub use state::{FlatNode, Tree, TreeId};
