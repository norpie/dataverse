//! Sort field editor modal for adding/editing order by entries.

use dataverse_lib::api::query::Direction;
use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{Autocomplete, AutocompleteState, Button, RadioGroup, RadioState, Text};

/// Modal for selecting a sort field and direction.
#[modal(default)]
pub struct SortFieldEditorModal {
    #[state(skip)]
    options: Vec<(String, String)>,
    #[state(skip)]
    initial_field: Option<String>,
    #[state(skip)]
    initial_direction: Option<Direction>,

    field: AutocompleteState<String>,
    direction: RadioState<String>,
}

impl SortFieldEditorModal {
    /// Create with pre-fetched field options: (logical_name, display_label).
    pub fn new(options: Vec<(String, String)>) -> Self {
        Self {
            options,
            ..Default::default()
        }
    }

    /// Create pre-filled with an existing sort field for editing.
    pub fn with_sort(options: Vec<(String, String)>, field: String, direction: Direction) -> Self {
        Self {
            options,
            initial_field: Some(field),
            initial_direction: Some(direction),
            ..Default::default()
        }
    }
}

#[modal_impl]
impl SortFieldEditorModal {
    fn default_result(&self) -> Option<(String, Direction)> {
        None
    }

    #[on_start]
    async fn on_start(&self, mx: &ModalContext<Option<(String, Direction)>>) {
        let field_state = if let Some(field) = &self.initial_field {
            AutocompleteState::new(self.options.clone()).with_value(field.clone())
        } else {
            AutocompleteState::new(self.options.clone())
        };
        self.field.set(field_state);

        let dir_value = match self.initial_direction {
            Some(Direction::Desc) => "desc",
            _ => "asc",
        };
        self.direction.set(
            RadioState::new([
                ("asc".to_string(), "Ascending".to_string()),
                ("desc".to_string(), "Descending".to_string()),
            ])
            .with_value(dir_value.to_string()),
        );
        mx.focus("sort-field-autocomplete");
    }

    #[keybinds]
    fn keys() {
        bind("escape", cancel);
    }

    #[handler]
    async fn cancel(&self, mx: &ModalContext<Option<(String, Direction)>>) {
        mx.close(None);
    }

    #[handler]
    async fn confirm(&self, mx: &ModalContext<Option<(String, Direction)>>) {
        let field = self.field.with_ref(|s| s.value().cloned());
        let Some(field_name) = field else {
            return;
        };

        let direction = self.direction.with_ref(|s| match s.value.as_deref() {
            Some("desc") => Direction::Desc,
            _ => Direction::Asc,
        });

        mx.close(Some((field_name, direction)));
    }

    fn element(&self) -> Element {
        let title = if self.initial_field.is_some() {
            "Edit Sort Field"
        } else {
            "Add Sort Field"
        };
        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                text (content: {title}) style (bold, fg: interact)
                text (content: "Field") style (fg: muted)
                autocomplete (state: self.field, id: "sort-field-autocomplete", placeholder: "Search fields...")
                text (content: "Direction") style (fg: muted)
                radio_group (state: self.direction, id: "sort-direction")
                row (width: fill, justify: between) {
                    button (label: "Cancel", hint: "esc", id: "cancel") on_activate: cancel()
                    button (label: "Ok", id: "ok") on_activate: confirm()
                }
            }
        }
    }
}
