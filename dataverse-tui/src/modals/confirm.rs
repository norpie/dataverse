//! Standardized confirmation modal.

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{Button, Text};

/// A standardized confirmation modal.
///
/// Returns `true` if confirmed (Ok), `false` if cancelled.
///
/// # Example
///
/// ```ignore
/// let confirmed = gx.modal(ConfirmModal::new("Delete this item?")).await;
/// if confirmed {
///     // do the thing
/// }
///
/// // With custom title:
/// let confirmed = gx.modal(
///     ConfirmModal::new("Are you sure?").title("Warning")
/// ).await;
/// ```
#[modal]
pub struct ConfirmModal {
    #[state(skip)]
    title: String,
    #[state(skip)]
    message: String,
}

impl ConfirmModal {
    /// Create a new confirmation modal with the given message.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            title: "Confirm".into(),
            message: message.into(),
            ..Default::default()
        }
    }

    /// Set a custom title.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }
}

#[modal_impl]
impl ConfirmModal {
    #[keybinds]
    fn keys() {
        bind("enter", "y", confirm);
        bind("escape", "n", cancel);
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
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                text (content: self.title.clone()) style (bold, fg: accent)
                text (content: self.message.clone())
                row (width: fill, justify: between) {
                    button (label: "Cancel", hint: "n", id: "cancel") on_activate: cancel()
                    button (label: "Ok", hint: "y", id: "ok") on_activate: confirm()
                }
            }
        }
    }
}
