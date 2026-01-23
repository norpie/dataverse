//! Save query modal for naming a saved query.

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{Button, Input, Text};

/// Modal for entering a name to save the query as.
/// Returns the chosen name, or None if cancelled.
#[modal]
pub struct SaveQueryModal {
    #[state(skip)]
    initial_name: String,

    name: String,
}

impl SaveQueryModal {
    /// Create with an optional pre-filled name.
    pub fn new(current_name: Option<String>) -> Self {
        Self {
            initial_name: current_name.unwrap_or_default(),
            ..Default::default()
        }
    }
}

#[modal_impl]
impl SaveQueryModal {
    fn default_result(&self) -> Option<String> {
        None
    }

    #[on_start]
    async fn on_start(&self, mx: &ModalContext<Option<String>>) {
        self.name.set(self.initial_name.clone());
        mx.focus("query-name");
    }

    #[keybinds]
    fn keys() {
        bind("escape", cancel);
    }

    #[handler]
    async fn cancel(&self, mx: &ModalContext<Option<String>>) {
        mx.close(None);
    }

    #[handler]
    async fn confirm(&self, mx: &ModalContext<Option<String>>) {
        let name = self.name.get();
        let name = name.trim().to_string();
        if name.is_empty() {
            return;
        }
        mx.close(Some(name));
    }

    fn element(&self) -> Element {
        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                text (content: "Save Query") style (bold, fg: interact)
                input (state: self.name, id: "query-name", label: "Name", placeholder: "My query...")
                    on_submit: confirm()
                row (width: fill, justify: between) {
                    button (label: "Cancel", hint: "esc", id: "cancel") on_activate: cancel()
                    button (label: "Save", id: "save") on_activate: confirm()
                }
            }
        }
    }
}
