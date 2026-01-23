//! Load query modal for selecting a saved query.

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{Autocomplete, AutocompleteState, Button, Text};

/// Modal for selecting a saved query to load.
/// Returns the selected query ID, or None if cancelled.
#[modal(size = Md)]
pub struct LoadQueryModal {
    #[state(skip)]
    options: Vec<(i64, String)>,

    queries: AutocompleteState<i64>,
}

impl LoadQueryModal {
    /// Create with pre-fetched query list: (id, display_label).
    pub fn new(options: Vec<(i64, String)>) -> Self {
        Self {
            options,
            ..Default::default()
        }
    }
}

#[modal_impl]
impl LoadQueryModal {
    fn default_result(&self) -> Option<i64> {
        None
    }

    #[on_start]
    async fn on_start(&self, mx: &ModalContext<Option<i64>>) {
        self.queries
            .set(AutocompleteState::new(self.options.clone()));
        mx.focus("query-autocomplete");
    }

    #[keybinds]
    fn keys() {
        bind("escape", cancel);
    }

    #[handler]
    async fn cancel(&self, mx: &ModalContext<Option<i64>>) {
        mx.close(None);
    }

    #[handler]
    async fn on_select(&self, mx: &ModalContext<Option<i64>>) {
        let selected = self.queries.with_ref(|s| s.value().cloned());
        if selected.is_some() {
            mx.close(selected);
        }
    }

    fn element(&self) -> Element {
        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                text (content: "Load Query") style (bold, fg: interact)
                autocomplete (state: self.queries, id: "query-autocomplete", placeholder: "Search saved queries...")
                    on_select: on_select()
                row (width: fill, justify: between) {
                    button (label: "Cancel", hint: "esc", id: "cancel") on_activate: cancel()
                    button (label: "Load", id: "load") on_activate: on_select()
                }
            }
        }
    }
}
