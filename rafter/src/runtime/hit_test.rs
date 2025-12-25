//! Hit testing for mouse interactions.

use ratatui::layout::Rect;

/// Information about a clickable element's position
#[derive(Debug, Clone)]
pub struct HitBox {
    /// Element ID
    pub id: String,
    /// Bounding rectangle
    pub rect: Rect,
    /// Whether this element captures text input
    pub captures_input: bool,
}

/// Collection of hit boxes for the current frame
#[derive(Debug, Default)]
pub struct HitTestMap {
    /// Hit boxes in render order (later elements are on top)
    boxes: Vec<HitBox>,
}

impl HitTestMap {
    /// Create a new empty hit test map
    pub fn new() -> Self {
        Self::default()
    }

    /// Clear all hit boxes (call at start of each frame)
    pub fn clear(&mut self) {
        self.boxes.clear();
    }

    /// Register a hit box for an element
    pub fn register(&mut self, id: String, rect: Rect, captures_input: bool) {
        // Log registration of taskbar buttons for debugging
        if id.starts_with("taskbar-btn") {
            log::debug!(
                "HitMap register: id={}, rect=({}, {}, {}x{})",
                id,
                rect.x,
                rect.y,
                rect.width,
                rect.height
            );
        }
        self.boxes.push(HitBox {
            id,
            rect,
            captures_input,
        });
    }

    /// Find the element at a given position (returns topmost)
    pub fn hit_test(&self, x: u16, y: u16) -> Option<&HitBox> {
        // Iterate in reverse to find topmost element
        self.boxes.iter().rev().find(|hit_box| {
            x >= hit_box.rect.x
                && x < hit_box.rect.x + hit_box.rect.width
                && y >= hit_box.rect.y
                && y < hit_box.rect.y + hit_box.rect.height
        })
    }

    /// Get an iterator over all registered widget IDs.
    ///
    /// Used for animation cleanup to identify which widgets were rendered.
    pub fn widget_ids(&self) -> impl Iterator<Item = &str> {
        self.boxes.iter().map(|b| b.id.as_str())
    }
}
