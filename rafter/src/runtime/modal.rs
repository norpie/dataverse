//! Modal stack management and modal area calculations.

use ratatui::layout::Rect;

use crate::input::focus::FocusState;
use crate::input::keybinds::Keybinds;
use crate::layers::modal::{ModalDyn, ModalPosition, ModalSize};
use crate::node::Node;

use super::input::InputState;

/// A modal entry in the modal stack
pub struct ModalStackEntry {
    /// The modal itself (type-erased)
    pub modal: Box<dyn ModalDyn>,
    /// Focus state for this modal
    pub focus_state: FocusState,
    /// Input state for keybind sequences
    pub input_state: InputState,
    /// Cached keybinds
    pub keybinds: Keybinds,
}

/// Calculate the modal's render area based on position and size settings.
pub fn calculate_modal_area(
    screen: Rect,
    position: ModalPosition,
    size: ModalSize,
    page: &Node,
) -> Rect {
    // Calculate dimensions
    let (width, height) = match size {
        ModalSize::Auto => {
            // Use intrinsic size from page
            let w = page.intrinsic_width().min(screen.width.saturating_sub(4));
            let h = page.intrinsic_height().min(screen.height.saturating_sub(4));
            (w.max(10), h.max(3))
        }
        ModalSize::Sm => {
            // Small: 30% of screen
            let w = (screen.width as f32 * 0.30) as u16;
            let h = (screen.height as f32 * 0.30) as u16;
            (w.max(10), h.max(3))
        }
        ModalSize::Md => {
            // Medium: 50% of screen
            let w = (screen.width as f32 * 0.50) as u16;
            let h = (screen.height as f32 * 0.50) as u16;
            (w.max(10), h.max(3))
        }
        ModalSize::Lg => {
            // Large: 80% of screen
            let w = (screen.width as f32 * 0.80) as u16;
            let h = (screen.height as f32 * 0.80) as u16;
            (w.max(10), h.max(3))
        }
        ModalSize::Fixed { width, height } => (width.min(screen.width), height.min(screen.height)),
        ModalSize::Proportional { width, height } => {
            let w = (screen.width as f32 * width) as u16;
            let h = (screen.height as f32 * height) as u16;
            (w.max(10), h.max(3))
        }
    };

    // Calculate position
    let (x, y) = match position {
        ModalPosition::Centered => {
            let x = (screen.width.saturating_sub(width)) / 2;
            let y = (screen.height.saturating_sub(height)) / 2;
            (x, y)
        }
        ModalPosition::At { x, y } => (
            x.min(screen.width.saturating_sub(width)),
            y.min(screen.height.saturating_sub(height)),
        ),
    };

    Rect::new(x, y, width, height)
}
