//! Layout calculations for system overlays.

use ratatui::layout::Rect;

use crate::layers::system_overlay::{AnySystemOverlay, SystemOverlayPosition};

/// Calculated layout for an overlay with its assigned area.
pub struct OverlayLayout<'a> {
    /// Index of this overlay in the system_overlays vector.
    pub index: usize,
    /// Reference to the overlay.
    pub overlay: &'a dyn AnySystemOverlay,
    /// The area assigned to this overlay.
    pub area: Rect,
}

/// Result of calculating system overlay layout.
pub struct SystemOverlayLayout<'a> {
    /// Edge overlays that should be rendered (in order: top, left, right, bottom).
    pub edge_overlays: Vec<OverlayLayout<'a>>,
    /// Absolute overlays that should be rendered on top.
    pub absolute_overlays: Vec<OverlayLayout<'a>>,
    /// Remaining area for app content (after edge overlays).
    pub app_area: Rect,
}

/// Calculate layout for all system overlays.
///
/// Edge overlays stack inward from their respective edges:
/// - Top overlays stack downward (first registered = closest to top edge)
/// - Bottom overlays stack upward (first registered = closest to bottom edge)
/// - Left overlays stack rightward (first registered = closest to left edge)
/// - Right overlays stack leftward (first registered = closest to right edge)
///
/// Absolute overlays are positioned at their specified coordinates.
pub fn calculate_system_overlay_layout<'a>(
    overlays: &'a [Box<dyn AnySystemOverlay>],
    terminal_area: Rect,
) -> SystemOverlayLayout<'a> {
    let mut edge_overlays = Vec::new();
    let mut absolute_overlays = Vec::new();

    // Track remaining app area as we consume space for edge overlays
    let mut app_area = terminal_area;

    // First pass: collect and categorize overlays
    // We need to process edges in a specific order to stack correctly
    let mut top_overlays: Vec<(usize, &'a dyn AnySystemOverlay, u16)> = Vec::new();
    let mut bottom_overlays: Vec<(usize, &'a dyn AnySystemOverlay, u16)> = Vec::new();
    let mut left_overlays: Vec<(usize, &'a dyn AnySystemOverlay, u16)> = Vec::new();
    let mut right_overlays: Vec<(usize, &'a dyn AnySystemOverlay, u16)> = Vec::new();

    for (idx, overlay) in overlays.iter().enumerate() {
        match overlay.position() {
            SystemOverlayPosition::Top { height } => {
                top_overlays.push((idx, overlay.as_ref(), height));
            }
            SystemOverlayPosition::Bottom { height } => {
                bottom_overlays.push((idx, overlay.as_ref(), height));
            }
            SystemOverlayPosition::Left { width } => {
                left_overlays.push((idx, overlay.as_ref(), width));
            }
            SystemOverlayPosition::Right { width } => {
                right_overlays.push((idx, overlay.as_ref(), width));
            }
            SystemOverlayPosition::Absolute { x, y, width, height } => {
                // Absolute overlays don't affect app area
                let area = Rect::new(
                    terminal_area.x + x,
                    terminal_area.y + y,
                    width.min(terminal_area.width.saturating_sub(x)),
                    height.min(terminal_area.height.saturating_sub(y)),
                );
                absolute_overlays.push(OverlayLayout {
                    index: idx,
                    overlay: overlay.as_ref(),
                    area,
                });
            }
        }
    }

    // Process top overlays (stack downward)
    for (idx, overlay, height) in top_overlays {
        let height = height.min(app_area.height);
        if height == 0 {
            continue;
        }
        let area = Rect::new(app_area.x, app_area.y, app_area.width, height);
        edge_overlays.push(OverlayLayout { index: idx, overlay, area });
        // Shrink app area from top
        app_area.y += height;
        app_area.height = app_area.height.saturating_sub(height);
    }

    // Process bottom overlays (stack upward)
    for (idx, overlay, height) in bottom_overlays {
        let height = height.min(app_area.height);
        if height == 0 {
            continue;
        }
        let area = Rect::new(
            app_area.x,
            app_area.y + app_area.height - height,
            app_area.width,
            height,
        );
        log::debug!(
            "Bottom overlay: app_area=({}, {}, {}x{}), overlay_area=({}, {}, {}x{})",
            app_area.x, app_area.y, app_area.width, app_area.height,
            area.x, area.y, area.width, area.height
        );
        edge_overlays.push(OverlayLayout { index: idx, overlay, area });
        // Shrink app area from bottom
        app_area.height = app_area.height.saturating_sub(height);
    }

    // Process left overlays (stack rightward)
    for (idx, overlay, width) in left_overlays {
        let width = width.min(app_area.width);
        if width == 0 {
            continue;
        }
        let area = Rect::new(app_area.x, app_area.y, width, app_area.height);
        edge_overlays.push(OverlayLayout { index: idx, overlay, area });
        // Shrink app area from left
        app_area.x += width;
        app_area.width = app_area.width.saturating_sub(width);
    }

    // Process right overlays (stack leftward)
    for (idx, overlay, width) in right_overlays {
        let width = width.min(app_area.width);
        if width == 0 {
            continue;
        }
        let area = Rect::new(
            app_area.x + app_area.width - width,
            app_area.y,
            width,
            app_area.height,
        );
        edge_overlays.push(OverlayLayout { index: idx, overlay, area });
        // Shrink app area from right
        app_area.width = app_area.width.saturating_sub(width);
    }

    SystemOverlayLayout {
        edge_overlays,
        absolute_overlays,
        app_area,
    }
}
