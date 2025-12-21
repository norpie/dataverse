//! TreeItem trait for items that can be displayed in a Tree.

use crate::node::Node;

/// Trait for items that can be displayed in a Tree.
///
/// Implement this trait to define hierarchical data for the tree.
///
/// # Example
///
/// ```ignore
/// #[derive(Clone, Debug)]
/// struct FileNode {
///     name: String,
///     is_dir: bool,
///     children: Vec<FileNode>,
/// }
///
/// impl TreeItem for FileNode {
///     fn id(&self) -> String {
///         self.name.clone()
///     }
///
///     fn children(&self) -> Vec<Self> {
///         self.children.clone()
///     }
///
///     fn render(&self, focused: bool, selected: bool, depth: u16, expanded: bool) -> Node {
///         let icon = if self.is_dir {
///             if expanded { "▼ " } else { "▶ " }
///         } else {
///             "  "
///         };
///         let indent = "  ".repeat(depth as usize);
///         view! {
///             text (fg: if focused { "primary" } else { "text" }) {
///                 format!("{}{}{}", indent, icon, self.name)
///             }
///         }
///     }
/// }
/// ```
pub trait TreeItem: Send + Sync + Clone + 'static {
    /// Height of each node in rows (constant for virtualization).
    const HEIGHT: u16 = 1;

    /// Unique, stable identifier for this node.
    ///
    /// This ID must be unique across the entire tree and stable across
    /// updates to maintain expand/collapse and selection state.
    fn id(&self) -> String;

    /// Get child items. Return an empty vec for leaf nodes.
    fn children(&self) -> Vec<Self>;

    /// Whether this node can be expanded.
    ///
    /// Default implementation returns true if the node has children.
    fn is_expandable(&self) -> bool {
        !self.children().is_empty()
    }

    /// Render this node.
    ///
    /// # Arguments
    /// * `focused` - Whether this node has the cursor (keyboard focus)
    /// * `selected` - Whether this node is selected
    /// * `depth` - Indentation level (0 = root level)
    /// * `expanded` - Whether children are currently visible
    fn render(&self, focused: bool, selected: bool, depth: u16, expanded: bool) -> Node;
}
