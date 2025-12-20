//! Geometry utilities for rectangle operations.

use ratatui::layout::Rect;

/// Clipping context for viewport rendering.
///
/// Used when rendering content that may extend beyond a visible viewport,
/// such as scrollable containers.
#[derive(Debug, Clone, Copy)]
pub struct ClipRect {
    /// The visible viewport area.
    pub viewport: Rect,
    /// Horizontal offset into the content.
    pub offset_x: u16,
    /// Vertical offset into the content.
    pub offset_y: u16,
}

/// Check if two rectangles overlap.
pub fn rects_overlap(a: Rect, b: Rect) -> bool {
    a.x < b.x + b.width && a.x + a.width > b.x && a.y < b.y + b.height && a.y + a.height > b.y
}

/// Compute the intersection of two rectangles.
///
/// Returns a zero-sized rect if they don't overlap.
pub fn intersect_rects(a: Rect, b: Rect) -> Rect {
    let x = a.x.max(b.x);
    let y = a.y.max(b.y);
    let right = (a.x + a.width).min(b.x + b.width);
    let bottom = (a.y + a.height).min(b.y + b.height);

    if right > x && bottom > y {
        Rect::new(x, y, right - x, bottom - y)
    } else {
        Rect::new(0, 0, 0, 0)
    }
}
