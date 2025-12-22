//! Overlay system for floating UI elements.
//!
//! Overlays are floating UI elements that render above the normal content layer.
//! They are used for dropdowns, context menus, tooltips, and similar components.
//!
//! Unlike modals, overlays:
//! - Are owned by widgets (not the runtime)
//! - Position relative to an anchor element
//! - Auto-close on blur or outside click
//! - Don't trap focus (focus stays on owner widget)
//!
//! # Usage
//!
//! Widgets register overlays during their render phase:
//!
//! ```ignore
//! fn render(&self, frame: &mut Frame, area: Rect, focused: bool, ctx: &mut RenderContext<'_>) {
//!     // Render the trigger element
//!     render_trigger(frame, area, self, focused, ctx);
//!     
//!     // If open, register overlay for rendering
//!     if self.is_open() {
//!         ctx.register_overlay(OverlayRequest {
//!             owner_id: self.id_string(),
//!             content: build_dropdown_content(),
//!             anchor: area,
//!             position: OverlayPosition::Below,
//!         });
//!     }
//! }
//! ```

use ratatui::layout::Rect;

use crate::node::Node;

/// Position preference for overlay placement relative to anchor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OverlayPosition {
    /// Below the anchor element (default for dropdowns).
    /// Falls back to Above if insufficient space below.
    #[default]
    Below,
    /// Above the anchor element.
    /// Falls back to Below if insufficient space above.
    Above,
    /// At a specific cursor position (for context menus).
    AtCursor { x: u16, y: u16 },
}

/// A request from a widget to render an overlay.
///
/// Widgets create overlay requests during their render phase when they need
/// to display floating content. The runtime collects these requests and
/// renders them in the overlay layer after all normal content.
#[derive(Debug)]
pub struct OverlayRequest {
    /// ID of the widget that owns this overlay.
    /// Used for click-outside detection and blur handling.
    pub owner_id: String,
    /// Content to render in the overlay.
    pub content: Node,
    /// Anchor rectangle (the trigger element's screen position).
    /// The overlay positions relative to this rectangle.
    pub anchor: Rect,
    /// Preferred position relative to anchor.
    pub position: OverlayPosition,
}

impl OverlayRequest {
    /// Create a new overlay request.
    pub fn new(owner_id: impl Into<String>, content: Node, anchor: Rect) -> Self {
        Self {
            owner_id: owner_id.into(),
            content,
            anchor,
            position: OverlayPosition::default(),
        }
    }

    /// Set the preferred position.
    pub fn with_position(mut self, position: OverlayPosition) -> Self {
        self.position = position;
        self
    }
}

/// Calculate the screen position for an overlay.
///
/// Takes into account:
/// - The anchor element's position
/// - The preferred placement (below, above, at cursor)
/// - Screen bounds (flips if content doesn't fit)
///
/// # Arguments
///
/// * `screen` - Total screen area
/// * `anchor` - Anchor element's rectangle
/// * `content_size` - Size of the overlay content (width, height)
/// * `position` - Preferred position
///
/// # Returns
///
/// The calculated rectangle for the overlay.
pub fn calculate_overlay_position(
    screen: Rect,
    anchor: Rect,
    content_size: (u16, u16),
    position: OverlayPosition,
) -> Rect {
    let (width, height) = content_size;

    // Constrain dimensions to screen
    let width = width.min(screen.width);
    let height = height.min(screen.height);

    match position {
        OverlayPosition::Below => {
            // Try below first
            let y_below = anchor.y + anchor.height;
            let fits_below = y_below + height <= screen.y + screen.height;

            if fits_below {
                let x = constrain_x(anchor.x, width, screen);
                Rect::new(x, y_below, width, height)
            } else {
                // Fall back to above
                let y_above = anchor.y.saturating_sub(height);
                let x = constrain_x(anchor.x, width, screen);
                Rect::new(x, y_above, width, height)
            }
        }
        OverlayPosition::Above => {
            // Try above first
            let fits_above = anchor.y >= height;

            if fits_above {
                let y = anchor.y.saturating_sub(height);
                let x = constrain_x(anchor.x, width, screen);
                Rect::new(x, y, width, height)
            } else {
                // Fall back to below
                let y = anchor.y + anchor.height;
                let x = constrain_x(anchor.x, width, screen);
                Rect::new(x, y, width, height)
            }
        }
        OverlayPosition::AtCursor { x, y } => {
            // Position at cursor, constrained to screen
            let x = x.min(screen.x + screen.width.saturating_sub(width));
            let y = y.min(screen.y + screen.height.saturating_sub(height));
            Rect::new(x, y, width, height)
        }
    }
}

/// Constrain x position to fit within screen bounds.
fn constrain_x(x: u16, width: u16, screen: Rect) -> u16 {
    let max_x = screen.x + screen.width.saturating_sub(width);
    x.min(max_x).max(screen.x)
}

/// Active overlay information tracked by the runtime.
///
/// This is used for hit testing and determining which widget
/// owns an active overlay.
#[derive(Debug, Clone)]
pub struct ActiveOverlay {
    /// ID of the widget that owns this overlay.
    pub owner_id: String,
    /// Screen rectangle where the overlay is rendered.
    pub area: Rect,
}

impl ActiveOverlay {
    /// Create a new active overlay record.
    pub fn new(owner_id: impl Into<String>, area: Rect) -> Self {
        Self {
            owner_id: owner_id.into(),
            area,
        }
    }

    /// Check if a point is inside this overlay.
    pub fn contains(&self, x: u16, y: u16) -> bool {
        x >= self.area.x
            && x < self.area.x + self.area.width
            && y >= self.area.y
            && y < self.area.y + self.area.height
    }
}
