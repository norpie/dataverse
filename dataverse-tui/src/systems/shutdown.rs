use rafter::prelude::*;

use crate::modals::ConfirmModal;

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
        let confirmed = gx
            .modal(ConfirmModal::new("Are you sure you want to quit?").title("Quit?"))
            .await;
        if confirmed {
            gx.shutdown();
        }
    }
}
