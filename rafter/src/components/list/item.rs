//! ListItem trait for items that can be displayed in a List.

use crate::node::Node;

/// Trait for items that can be displayed in a List.
///
/// Implement this trait to define how your items render in the list.
pub trait ListItem: Send + Sync + Clone + 'static {
    /// Height of each item in rows (constant for all items of this type).
    const HEIGHT: u16 = 1;

    /// Render this item.
    ///
    /// # Arguments
    /// * `focused` - Whether this item has the cursor (keyboard focus)
    /// * `selected` - Whether this item is selected (for multi-select)
    fn render(&self, focused: bool, selected: bool) -> Node;
}
