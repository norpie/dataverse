//! Field picker modal for selecting entity attributes.

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{Autocomplete, AutocompleteState, Button, SelectionMode, Text};

/// Modal for selecting one or more entity fields.
#[modal(default, size = Md)]
pub struct FieldPickerModal {
    #[state(skip)]
    options: Vec<(String, String)>,
    #[state(skip)]
    initial_selected: Vec<String>,

    fields: AutocompleteState<String>,
}

impl FieldPickerModal {
    /// Create with pre-fetched field options: (logical_name, display_label).
    pub fn with_options(options: Vec<(String, String)>) -> Self {
        Self {
            options,
            ..Default::default()
        }
    }

    /// Create pre-filled with already-selected fields.
    pub fn with_selected(options: Vec<(String, String)>, selected: Vec<String>) -> Self {
        Self {
            options,
            initial_selected: selected,
            ..Default::default()
        }
    }
}

#[modal_impl]
impl FieldPickerModal {
    fn default_result(&self) -> Option<Vec<String>> {
        None
    }

    #[on_start]
    async fn on_start(&self, mx: &ModalContext<Option<Vec<String>>>) {
        let mut state =
            AutocompleteState::new(self.options.clone()).with_selection(SelectionMode::Multi);
        for field in &self.initial_selected {
            state = state.with_value(field.clone());
        }
        self.fields.set(state);
        mx.focus("field-autocomplete");
    }

    #[keybinds]
    fn keys() {
        bind("escape", cancel);
    }

    #[handler]
    async fn cancel(&self, mx: &ModalContext<Option<Vec<String>>>) {
        mx.close(None);
    }

    #[handler]
    async fn confirm(&self, mx: &ModalContext<Option<Vec<String>>>) {
        let selected: Vec<String> = self
            .fields
            .with_ref(|s| s.selected_values().cloned().collect());
        mx.close(Some(selected));
    }

    #[derived]
    fn selected_count(&self) -> usize {
        self.fields.with_ref(|s| s.selection.selected.len())
    }

    fn element(&self) -> Element {
        let count = self.selected_count();

        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                text (content: "Select Fields") style (bold, fg: interact)
                autocomplete (state: self.fields, id: "field-autocomplete", placeholder: "Search fields...")
                text (content: {format!("{} selected", count)}) style (fg: muted)
                row (width: fill, justify: between) {
                    button (label: "Cancel", hint: "esc", id: "cancel") on_activate: cancel()
                    button (label: "Ok", id: "ok") on_activate: confirm()
                }
            }
        }
    }
}
