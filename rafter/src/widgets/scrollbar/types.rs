//! Scrollbar types used by scrollable widgets.

use crate::styling::color::StyleColor;

/// Scrollbar visibility configuration.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum ScrollbarVisibility {
    /// Show scrollbar only when content overflows.
    #[default]
    Auto,
    /// Always show scrollbar.
    Always,
    /// Never show scrollbar.
    Never,
}

/// Scrollbar appearance configuration.
#[derive(Debug, Default, Clone)]
pub struct ScrollbarConfig {
    /// Horizontal scrollbar visibility.
    pub horizontal: ScrollbarVisibility,
    /// Vertical scrollbar visibility.
    pub vertical: ScrollbarVisibility,
    /// Track (background) color.
    pub track_color: Option<StyleColor>,
    /// Handle (draggable part) color.
    pub handle_color: Option<StyleColor>,
}

/// Scrollbar geometry for hit testing and rendering.
#[derive(Debug, Clone, Copy, Default)]
pub struct ScrollbarGeometry {
    /// X position of the scrollbar track.
    pub x: u16,
    /// Y position of the scrollbar track.
    pub y: u16,
    /// Width of the scrollbar (1 for vertical, track length for horizontal).
    pub width: u16,
    /// Height of the scrollbar (track length for vertical, 1 for horizontal).
    pub height: u16,
    /// Position of the handle within the track (0-based).
    pub handle_pos: u16,
    /// Size of the handle.
    pub handle_size: u16,
}

impl ScrollbarGeometry {
    /// Check if a point is within the scrollbar track.
    pub fn contains(&self, x: u16, y: u16) -> bool {
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height
    }

    /// Check if a point is on the handle.
    pub fn handle_contains(&self, x: u16, y: u16, vertical: bool) -> bool {
        if !self.contains(x, y) {
            return false;
        }
        if vertical {
            let rel_y = y - self.y;
            rel_y >= self.handle_pos && rel_y < self.handle_pos + self.handle_size
        } else {
            let rel_x = x - self.x;
            rel_x >= self.handle_pos && rel_x < self.handle_pos + self.handle_size
        }
    }

    /// Convert a position on the track to a scroll ratio (0.0 - 1.0).
    /// Centers the handle on the click position.
    pub fn position_to_ratio(&self, x: u16, y: u16, vertical: bool) -> f32 {
        self.position_to_ratio_with_offset(x, y, vertical, self.handle_size / 2)
    }

    /// Convert a position on the track to a scroll ratio, with a custom offset within the handle.
    /// `grab_offset` is where within the handle the user grabbed (0 = top/left edge).
    pub fn position_to_ratio_with_offset(
        &self,
        x: u16,
        y: u16,
        vertical: bool,
        grab_offset: u16,
    ) -> f32 {
        if vertical {
            let track_size = self.height.saturating_sub(self.handle_size);
            if track_size == 0 {
                return 0.0;
            }
            let rel_y = y.saturating_sub(self.y).saturating_sub(grab_offset);
            (rel_y as f32 / track_size as f32).clamp(0.0, 1.0)
        } else {
            let track_size = self.width.saturating_sub(self.handle_size);
            if track_size == 0 {
                return 0.0;
            }
            let rel_x = x.saturating_sub(self.x).saturating_sub(grab_offset);
            (rel_x as f32 / track_size as f32).clamp(0.0, 1.0)
        }
    }
}

/// Internal drag state for scrollbar dragging.
#[derive(Debug, Clone, Copy)]
pub struct ScrollbarDrag {
    /// Whether dragging the vertical scrollbar (false = horizontal).
    pub is_vertical: bool,
    /// Offset within the handle where the user grabbed.
    pub grab_offset: u16,
}
