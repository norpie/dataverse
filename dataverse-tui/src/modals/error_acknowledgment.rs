//! Simple error acknowledgment modal with a title and wrapping message.

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{Button, Text};

/// A simple error acknowledgment modal.
///
/// Displays a title and a wrapping error message with a single close button.
/// Use this instead of a toast when the error requires user acknowledgment.
///
/// # Example
///
/// ```ignore
/// gx.modal(ErrorAcknowledgmentModal::new("Connection Failed", "Could not reach the server.")).await;
/// ```
#[modal(size = Md)]
pub struct ErrorAcknowledgmentModal {
    #[state(skip)]
    title: String,
    #[state(skip)]
    message: String,
}

#[modal_impl]
impl ErrorAcknowledgmentModal {
    fn default_result(&self) -> () {
        ()
    }

    #[keybinds]
    fn keys() {
        bind("escape", close);
    }

    #[handler]
    async fn close(&self, mx: &ModalContext<()>) {
        mx.close(());
    }

    fn element(&self) -> Element {
        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                text (content: self.title.clone()) style (bold, fg: interact)
                text (content: self.message.clone(), text_wrap: word_wrap, height: fill) style (fg: primary)
                row (width: fill, justify: center) {
                    button (label: "Close", hint: "esc", id: "close") on_activate: close()
                }
            }
        }
    }
}
