//! Scrollbar state trait for components that support scrolling.

use super::types::{ScrollbarConfig, ScrollbarDrag, ScrollbarGeometry};

/// Trait for components that support scrollbar functionality.
///
/// This trait provides a unified interface for scrollable components,
/// allowing them to share scrollbar rendering and event handling logic.
pub trait ScrollbarState {
    // -------------------------------------------------------------------------
    // Configuration
    // -------------------------------------------------------------------------

    /// Get the scrollbar configuration.
    fn scrollbar_config(&self) -> ScrollbarConfig;

    /// Set the scrollbar configuration.
    fn set_scrollbar_config(&self, config: ScrollbarConfig);

    // -------------------------------------------------------------------------
    // Scroll position
    // -------------------------------------------------------------------------

    /// Get the current vertical scroll offset.
    fn scroll_offset_y(&self) -> u16;

    /// Get the current horizontal scroll offset.
    fn scroll_offset_x(&self) -> u16 {
        0 // Default: no horizontal scrolling
    }

    /// Scroll to an absolute vertical position.
    fn scroll_to_y(&self, y: u16);

    /// Scroll to an absolute horizontal position.
    fn scroll_to_x(&self, _x: u16) {
        // Default: no horizontal scrolling
    }

    /// Scroll by a relative amount.
    fn scroll_by(&self, dx: i16, dy: i16);

    /// Scroll to the top.
    fn scroll_to_top(&self);

    /// Scroll to the bottom.
    fn scroll_to_bottom(&self);

    // -------------------------------------------------------------------------
    // Content/viewport dimensions
    // -------------------------------------------------------------------------

    /// Get the total content height.
    fn content_height(&self) -> u16;

    /// Get the total content width.
    fn content_width(&self) -> u16 {
        0 // Default: no horizontal scrolling
    }

    /// Get the viewport height.
    fn viewport_height(&self) -> u16;

    /// Get the viewport width.
    fn viewport_width(&self) -> u16 {
        0 // Default: no horizontal scrolling
    }

    // -------------------------------------------------------------------------
    // Computed properties
    // -------------------------------------------------------------------------

    /// Get the maximum vertical scroll offset.
    fn max_scroll_y(&self) -> u16 {
        self.content_height().saturating_sub(self.viewport_height())
    }

    /// Get the maximum horizontal scroll offset.
    fn max_scroll_x(&self) -> u16 {
        self.content_width().saturating_sub(self.viewport_width())
    }

    /// Check if vertical scrolling is needed.
    fn needs_vertical_scrollbar(&self) -> bool {
        self.content_height() > self.viewport_height()
    }

    /// Check if horizontal scrolling is needed.
    fn needs_horizontal_scrollbar(&self) -> bool {
        self.content_width() > self.viewport_width()
    }

    // -------------------------------------------------------------------------
    // Scrollbar geometry (set by renderer, used for hit testing)
    // -------------------------------------------------------------------------

    /// Get the vertical scrollbar geometry.
    fn vertical_scrollbar(&self) -> Option<ScrollbarGeometry>;

    /// Set the vertical scrollbar geometry.
    fn set_vertical_scrollbar(&self, geometry: Option<ScrollbarGeometry>);

    /// Get the horizontal scrollbar geometry.
    fn horizontal_scrollbar(&self) -> Option<ScrollbarGeometry> {
        None // Default: no horizontal scrollbar
    }

    /// Set the horizontal scrollbar geometry.
    fn set_horizontal_scrollbar(&self, _geometry: Option<ScrollbarGeometry>) {
        // Default: no horizontal scrollbar
    }

    // -------------------------------------------------------------------------
    // Drag state
    // -------------------------------------------------------------------------

    /// Get current drag state.
    fn drag(&self) -> Option<ScrollbarDrag>;

    /// Set current drag state.
    fn set_drag(&self, drag: Option<ScrollbarDrag>);

    // -------------------------------------------------------------------------
    // Ratio-based scrolling (for scrollbar dragging)
    // -------------------------------------------------------------------------

    /// Scroll to a position based on a ratio (0.0 - 1.0).
    fn scroll_to_ratio(&self, x_ratio: Option<f32>, y_ratio: Option<f32>) {
        if let Some(ratio) = y_ratio {
            let max_y = self.max_scroll_y();
            let y = (ratio * max_y as f32).round() as u16;
            self.scroll_to_y(y);
        }
        if let Some(ratio) = x_ratio {
            let max_x = self.max_scroll_x();
            let x = (ratio * max_x as f32).round() as u16;
            self.scroll_to_x(x);
        }
    }
}
