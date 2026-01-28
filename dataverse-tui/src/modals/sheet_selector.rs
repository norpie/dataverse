//! Sheet selector modal for Excel files.

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{Button, Select, SelectState, Text};

/// Modal for selecting an Excel sheet.
#[modal(default, size = Auto)]
pub struct SheetSelectorModal {
    /// Available sheets.
    #[state(skip)]
    sheets: Vec<String>,

    /// Select state.
    select: SelectState<String>,
}

impl SheetSelectorModal {
    pub fn new(sheets: Vec<String>) -> Self {
        let options: Vec<(String, String)> = sheets
            .iter()
            .map(|name| (name.clone(), name.clone()))
            .collect();

        Self {
            sheets,
            select: State::new(SelectState::new(options)),
            ..Default::default()
        }
    }
}

#[modal_impl(Result = Option<String>)]
impl SheetSelectorModal {
    fn default_result(&self) -> Option<String> {
        None
    }

    #[on_start]
    async fn on_start(&self, mx: &ModalContext<Option<String>>) {
        mx.focus("sheet-select");
    }

    #[keybinds]
    fn keybinds() {
        bind("escape", cancel);
    }

    #[handler]
    async fn cancel(&self, mx: &ModalContext<Option<String>>) {
        mx.close(None);
    }

    #[handler]
    async fn confirm(&self, mx: &ModalContext<Option<String>>) {
        let selected = self.select.with_ref(|s| s.value().cloned());
        mx.close(selected);
    }

    fn element(&self) -> Element {
        let sheet_count = self.sheets.len();
        let count_text = format!("{} sheets available", sheet_count);

        page! {
            column (padding: (1, 2), gap: 1, width: fill) style (bg: surface) {
                // Header
                column {
                    text (content: "Select Sheet") style (bold, fg: interact)
                    text (content: count_text) style (fg: muted)
                }

                // Select widget
                select (state: self.select, id: "sheet-select", label: "Sheet")

                // Footer
                row (width: fill, justify: between) {
                    button (label: "Cancel", hint: "esc", id: "cancel") on_activate: cancel()
                    button (label: "Select", id: "select") on_activate: confirm()
                }
            }
        }
    }
}
