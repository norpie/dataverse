mod content;
mod node;

pub use content::{Content, CustomContent};
pub use node::Element;

/// Find an element by ID in the tree.
pub fn find_element<'a>(root: &'a Element, id: &str) -> Option<&'a Element> {
    if root.id == id {
        return Some(root);
    }

    if let Content::Children(children) = &root.content {
        for child in children {
            if let Some(found) = find_element(child, id) {
                return Some(found);
            }
        }
    }

    None
}
