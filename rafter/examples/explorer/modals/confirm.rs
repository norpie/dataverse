//! Confirmation modal dialog.

use rafter::prelude::*;

/// A confirmation modal that asks the user to confirm an action.
#[modal]
pub struct ConfirmModal {
    #[state(skip)]
    message: String,
    #[state(skip)]
    confirm_text: String,
    #[state(skip)]
    cancel_text: String,
}

impl ConfirmModal {
    /// Create a new confirmation modal with the given message.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            confirm_text: "Yes".to_string(),
            cancel_text: "No".to_string(),
        }
    }

    /// Set custom button text.
    #[allow(dead_code)]
    pub fn with_buttons(mut self, confirm: impl Into<String>, cancel: impl Into<String>) -> Self {
        self.confirm_text = confirm.into();
        self.cancel_text = cancel.into();
        self
    }
}

#[modal_impl]
impl ConfirmModal {
    #[keybinds]
    fn keys() -> Keybinds {
        keybinds! {
            "y" | "enter" => confirm,
            "n" | "escape" => cancel,
        }
    }

    #[handler]
    async fn confirm(&self, mx: &ModalContext<bool>) {
        mx.close(true);
    }

    #[handler]
    async fn cancel(&self, mx: &ModalContext<bool>) {
        mx.close(false);
    }

    fn page(&self) -> Node {
        let message = self.message.clone();
        let confirm_label = format!("{} [y]", self.confirm_text);
        let cancel_label = format!("{} [n]", self.cancel_text);

        page! {
            column (padding: 2, gap: 1, bg: surface) {
                text (bold, fg: warning) { "Confirm" }
                text { message }
                row (gap: 2) {
                    button(label: cancel_label, id: "cancel", on_click: cancel)
                    button(label: confirm_label, id: "confirm", on_click: confirm)
                }
            }
        }
    }
}
