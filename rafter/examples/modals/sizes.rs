//! Size preset modals demonstrating all ModalSize variants.

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{Button, Text};

// ============================================================================
// Auto Size Modal (default - fits content)
// ============================================================================

#[modal]
pub struct AutoModal;

#[modal_impl]
impl AutoModal {
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
            column (padding: 2, gap: 1) style (bg: surface) {
                text (content: "Auto Size Modal") style (bold, fg: primary)
                text (content: "Size: Auto (default)") style (fg: muted)
                text (content: "Fits to content automatically")
                button (label: "Close [Esc]", id: "close") on_activate: close()
            }
        }
    }
}

// ============================================================================
// Small Size Modal (30% of screen)
// ============================================================================

#[modal(size = Sm)]
pub struct SmModal;

#[modal_impl]
impl SmModal {
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
            column (padding: 2, gap: 1) style (bg: surface) {
                text (content: "Small Modal") style (bold, fg: primary)
                text (content: "Size: Sm (30% of screen)") style (fg: muted)
                text (content: "Good for simple confirmations")
                button (label: "Close [Esc]", id: "close") on_activate: close()
            }
        }
    }
}

// ============================================================================
// Medium Size Modal (50% of screen)
// ============================================================================

#[modal(size = Md)]
pub struct MdModal;

#[modal_impl]
impl MdModal {
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
            column (padding: 2, gap: 1) style (bg: surface) {
                text (content: "Medium Modal") style (bold, fg: primary)
                text (content: "Size: Md (50% of screen)") style (fg: muted)
                text (content: "Balanced size for forms and dialogs")
                text (content: "Has room for multiple inputs")
                button (label: "Close [Esc]", id: "close") on_activate: close()
            }
        }
    }
}

// ============================================================================
// Large Size Modal (80% of screen)
// ============================================================================

#[modal(size = Lg)]
pub struct LgModal;

#[modal_impl]
impl LgModal {
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
            column (padding: 2, gap: 1) style (bg: surface) {
                text (content: "Large Modal") style (bold, fg: primary)
                text (content: "Size: Lg (80% of screen)") style (fg: muted)
                text (content: "Maximum preset size")
                text (content: "Good for complex forms, previews, or detailed content")
                text (content: "Lots of room for content here...")
                button (label: "Close [Esc]", id: "close") on_activate: close()
            }
        }
    }
}

// ============================================================================
// Fixed Size Modal (exact dimensions)
// ============================================================================

#[modal(size = Fixed { width: 50, height: 12 })]
pub struct FixedModal;

#[modal_impl]
impl FixedModal {
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
            column (padding: 2, gap: 1) style (bg: surface) {
                text (content: "Fixed Size Modal") style (bold, fg: primary)
                text (content: "Size: Fixed { width: 50, height: 12 }") style (fg: muted)
                text (content: "Exact cell dimensions")
                text (content: "Useful when you need precise control")
                button (label: "Close [Esc]", id: "close") on_activate: close()
            }
        }
    }
}

// ============================================================================
// Proportional Size Modal (percentage of screen)
// ============================================================================

#[modal(size = Proportional { width: 0.6, height: 0.4 })]
pub struct ProportionalModal;

#[modal_impl]
impl ProportionalModal {
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
            column (padding: 2, gap: 1) style (bg: surface) {
                text (content: "Proportional Size Modal") style (bold, fg: primary)
                text (content: "Size: Proportional { width: 0.6, height: 0.4 }") style (fg: muted)
                text (content: "60% width, 40% height of screen")
                text (content: "Scales with terminal size")
                button (label: "Close [Esc]", id: "close") on_activate: close()
            }
        }
    }
}
