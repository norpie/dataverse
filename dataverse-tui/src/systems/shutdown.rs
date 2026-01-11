use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{Button, Text};

#[modal]
struct ShutdownConfirmModal;

#[modal_impl]
impl ShutdownConfirmModal {
    #[keybinds]
    fn keys() {
        bind("y", "enter", confirm);
        bind("n", "escape", cancel);
    }

    #[handler]
    async fn confirm(&self, mx: &ModalContext<bool>) {
        mx.close(true);
    }

    #[handler]
    async fn cancel(&self, mx: &ModalContext<bool>) {
        mx.close(false);
    }

    fn element(&self) -> Element {
        page! {
            column (padding: 2, gap: 1) style (bg: surface) {
                text (content: "Quit?") style (bold, fg: warning)
                text (content: "Are you sure you want to quit?")
                row (gap: 2) {
                    button (label: "[N]o", id: "no") on_activate: cancel()
                    button (label: "[Y]es", id: "yes") on_activate: confirm()
                }
            }
        }
    }
}

#[system]
pub struct Shutdown;

#[system_impl]
impl Shutdown {
    #[keybinds]
    fn keys() {
        bind("ctrl+q", quit);
    }

    #[handler]
    async fn quit(&self, gx: &GlobalContext) {
        if gx.modal(ShutdownConfirmModal::default()).await {
            gx.shutdown();
        }
    }
}
