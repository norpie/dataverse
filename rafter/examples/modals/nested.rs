//! Nested modals demonstrating modal stacking.

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{Button, Text};

// ============================================================================
// Inner Confirmation Modal
// ============================================================================

#[modal]
pub struct InnerConfirmModal;

#[modal_impl(Result = bool)]
impl InnerConfirmModal {
    #[keybinds]
    fn keys() {
        bind("y", "enter", confirm);
        bind("n", "escape", deny);
    }

    #[handler]
    async fn confirm(&self, mx: &ModalContext<bool>) {
        mx.close(true);
    }

    #[handler]
    async fn deny(&self, mx: &ModalContext<bool>) {
        mx.close(false);
    }

    fn element(&self) -> Element {
        page! {
            column (padding: 2, gap: 1) style (bg: error) {
                text (content: "Are you REALLY sure?") style (bold, fg: on_error)
                text (content: "This is a nested modal!") style (fg: on_error)
                row (gap: 2) {
                    button (label: "No [N]", id: "no") on_activate: deny()
                    button (label: "Yes [Y]", id: "yes") on_activate: confirm()
                }
            }
        }
    }
}

// ============================================================================
// Outer Modal (opens nested modal)
// ============================================================================

#[modal(size = Md)]
pub struct OuterModal {
    confirmed_count: i32,
}

#[modal_impl(Result = i32)]
impl OuterModal {
    #[keybinds]
    fn keys() {
        bind("escape", close);
        bind("c", confirm_action);
    }

    #[handler]
    async fn confirm_action(&self, cx: &AppContext) {
        // Open nested modal
        let really_sure = cx.modal(InnerConfirmModal::default()).await;
        if really_sure {
            self.confirmed_count.update(|c| *c += 1);
        }
    }

    #[handler]
    async fn close(&self, mx: &ModalContext<i32>) {
        mx.close(self.confirmed_count.get());
    }

    fn element(&self) -> Element {
        let count = self.confirmed_count.get().to_string();

        page! {
            column (padding: 2, gap: 1) style (bg: surface) {
                text (content: "Outer Modal") style (bold, fg: primary)
                text (content: "Press C to open a nested confirmation modal") style (fg: muted)
                row (gap: 1) {
                    text (content: "Confirmed actions:") style (fg: muted)
                    text (content: {count}) style (bold, fg: success)
                }
                text (content: "Each confirmation opens a nested modal")
                row (gap: 2) {
                    button (label: "Confirm [C]", id: "confirm") on_activate: confirm_action()
                    button (label: "Close [Esc]", id: "close") on_activate: close()
                }
            }
        }
    }
}

// ============================================================================
// Deep Nested Modal (3 levels)
// ============================================================================

#[modal]
pub struct Level3Modal;

#[modal_impl(Result = bool)]
impl Level3Modal {
    #[keybinds]
    fn keys() {
        bind("y", "enter", yes);
        bind("n", "escape", no);
    }

    #[handler]
    async fn yes(&self, mx: &ModalContext<bool>) {
        mx.close(true);
    }

    #[handler]
    async fn no(&self, mx: &ModalContext<bool>) {
        mx.close(false);
    }

    fn element(&self) -> Element {
        page! {
            column (padding: 2, gap: 1) style (bg: error) {
                text (content: "Level 3 - Final Confirmation") style (bold, fg: on_error)
                text (content: "This is the deepest level!") style (fg: on_error)
                row (gap: 2) {
                    button (label: "No", id: "no") on_activate: no()
                    button (label: "Yes", id: "yes") on_activate: yes()
                }
            }
        }
    }
}

#[modal(size = Sm)]
pub struct Level2Modal;

#[modal_impl(Result = bool)]
impl Level2Modal {
    #[keybinds]
    fn keys() {
        bind("enter", proceed);
        bind("escape", cancel);
    }

    #[handler]
    async fn proceed(&self, cx: &AppContext, mx: &ModalContext<bool>) {
        let confirmed = cx.modal(Level3Modal::default()).await;
        if confirmed {
            mx.close(true);
        }
    }

    #[handler]
    async fn cancel(&self, mx: &ModalContext<bool>) {
        mx.close(false);
    }

    fn element(&self) -> Element {
        page! {
            column (padding: 2, gap: 1) style (bg: warning) {
                text (content: "Level 2 - Continue?") style (bold, fg: on_warning)
                text (content: "Press Enter to go deeper") style (fg: on_warning)
                row (gap: 2) {
                    button (label: "Cancel [Esc]", id: "cancel") on_activate: cancel()
                    button (label: "Continue [Enter]", id: "proceed") on_activate: proceed()
                }
            }
        }
    }
}

#[modal(size = Md)]
pub struct Level1Modal;

#[modal_impl(Result = bool)]
impl Level1Modal {
    #[keybinds]
    fn keys() {
        bind("enter", proceed);
        bind("escape", cancel);
    }

    #[handler]
    async fn proceed(&self, cx: &AppContext, mx: &ModalContext<bool>) {
        let confirmed = cx.modal(Level2Modal::default()).await;
        if confirmed {
            mx.close(true);
        }
    }

    #[handler]
    async fn cancel(&self, mx: &ModalContext<bool>) {
        mx.close(false);
    }

    fn element(&self) -> Element {
        page! {
            column (padding: 2, gap: 1) style (bg: surface) {
                text (content: "Level 1 - Deep Nesting Demo") style (bold, fg: primary)
                text (content: "This will open 3 levels of nested modals") style (fg: muted)
                text (content: "Level 1 -> Level 2 -> Level 3")
                row (gap: 2) {
                    button (label: "Cancel [Esc]", id: "cancel") on_activate: cancel()
                    button (label: "Start [Enter]", id: "proceed") on_activate: proceed()
                }
            }
        }
    }
}
