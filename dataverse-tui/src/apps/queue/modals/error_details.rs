//! Error details modal - displays full error text from a failed execution.

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{Button, Text};

/// Modal that displays the full error text from a failed queue item execution.
#[modal(default, size = Lg)]
pub struct ErrorDetailsModal {
    #[state(skip)]
    error_text: String,
}

impl ErrorDetailsModal {
    pub fn with_error(error_text: String) -> Self {
        Self {
            error_text,
            ..Default::default()
        }
    }
}

#[modal_impl]
impl ErrorDetailsModal {
    fn default_result(&self) {}

    #[keybinds]
    fn keys() {
        bind("escape", close);
    }

    #[handler]
    async fn close(&self, mx: &ModalContext<()>) {
        mx.close(());
    }

    fn element(&self) -> Element {
        let error = self.error_text.clone();

        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                text (content: "Error Details") style (bold, fg: interact)
                column (id: "error-scroll", height: fill, width: fill, overflow: scroll) {
                    text (content: {error}) style (fg: error)
                }
                row (width: fill, justify: end) {
                    button (label: "Close", hint: "esc", id: "close") on_activate: close()
                }
            }
        }
    }
}
