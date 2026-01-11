//! Positioned modal demonstrating ModalPosition and on_start lifecycle.

use log::info;
use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{Button, Text};

// ============================================================================
// Positioned Modal (absolute position)
// ============================================================================

#[modal(position = At { x: 5, y: 3 })]
pub struct PositionedModal {
    started: bool,
}

#[modal_impl]
impl PositionedModal {
    /// Called when the modal is first shown.
    /// Demonstrates the on_start lifecycle hook.
    async fn on_start(&self) {
        info!("[PositionedModal] on_start called!");
        self.started.set(true);
    }

    #[keybinds]
    fn keys() {
        bind("escape", "enter", close);
    }

    #[handler]
    async fn close(&self, mx: &ModalContext<()>) {
        mx.close(());
    }

    fn element(&self) -> Element {
        let started_text = if self.started.get() {
            "on_start was called!"
        } else {
            "on_start not yet called"
        };

        page! {
            column (padding: 2, gap: 1) style (bg: surface) {
                text (content: "Positioned Modal") style (bold, fg: primary)
                text (content: "Position: At { x: 5, y: 3 }") style (fg: muted)
                text (content: "Absolute position from top-left")
                row (gap: 1) {
                    text (content: "Lifecycle:") style (fg: muted)
                    text (content: {started_text}) style (fg: success)
                }
                button (label: "Close [Esc]", id: "close") on_activate: close()
            }
        }
    }
}

// ============================================================================
// Corner Modal (demonstrates another position)
// ============================================================================

#[modal(position = At { x: 2, y: 2 }, size = Fixed { width: 35, height: 8 })]
pub struct CornerModal;

#[modal_impl]
impl CornerModal {
    async fn on_start(&self) {
        info!("[CornerModal] on_start - modal appeared in corner!");
    }

    #[keybinds]
    fn keys() {
        bind("escape", "enter", close);
    }

    #[handler]
    async fn close(&self, mx: &ModalContext<()>) {
        mx.close(());
    }

    fn element(&self) -> Element {
        page! {
            column (padding: 1, gap: 1) style (bg: surface) {
                text (content: "Corner Modal") style (bold, fg: primary)
                text (content: "Top-left corner") style (fg: muted)
                button (label: "Close", id: "close") on_activate: close()
            }
        }
    }
}
