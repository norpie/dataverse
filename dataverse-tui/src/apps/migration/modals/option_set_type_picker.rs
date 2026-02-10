//! Modal for picking an OptionSet field type (kind + option set name).

use dataverse_lib::model::FieldType;
use dataverse_lib::model::metadata::AttributeType;
use rafter::page;
use rafter::prelude::*;
use rafter::widgets::Autocomplete;
use rafter::widgets::AutocompleteState;
use rafter::widgets::Button;
use rafter::widgets::List;
use rafter::widgets::ListItem;
use rafter::widgets::ListState;
use rafter::widgets::Text;
use tuidom::Element;

/// Option set kind entry for the kind selector list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OptionSetKindEntry {
    kind: AttributeType,
}

impl OptionSetKindEntry {
    fn label(&self) -> &'static str {
        match self.kind {
            AttributeType::Picklist => "Picklist",
            AttributeType::State => "State",
            AttributeType::Status => "Status",
            AttributeType::MultiSelectPicklist => "Multi-Select Picklist",
            _ => "Unknown",
        }
    }

    fn description(&self) -> &'static str {
        match self.kind {
            AttributeType::Picklist => "Single-select choice field",
            AttributeType::State => "Record state (Active, Inactive, etc.)",
            AttributeType::Status => "Record status reason",
            AttributeType::MultiSelectPicklist => "Multi-select choice field",
            _ => "",
        }
    }

    fn all() -> Vec<Self> {
        vec![
            Self {
                kind: AttributeType::Picklist,
            },
            Self {
                kind: AttributeType::State,
            },
            Self {
                kind: AttributeType::Status,
            },
            Self {
                kind: AttributeType::MultiSelectPicklist,
            },
        ]
    }
}

impl ListItem for OptionSetKindEntry {
    type Key = String;

    fn key(&self) -> String {
        self.label().to_string()
    }

    fn render(&self) -> Element {
        Element::text(self.label())
    }
}

/// Modal for picking an option set field type.
///
/// The caller provides a list of available global option set names (pre-fetched).
/// Returns `Some(FieldType::OptionSet { kind, name })` on confirmation, `None` on cancel.
#[modal(size = Md)]
pub struct OptionSetTypePickerModal {
    kind_list: ListState<OptionSetKindEntry>,
    name_autocomplete: AutocompleteState<String>,
}

impl OptionSetTypePickerModal {
    /// Create a new option set type picker modal.
    ///
    /// `option_set_names` is the pre-fetched list of global option set names.
    pub fn new_modal(option_set_names: Vec<String>) -> Self {
        let options: Vec<(String, String)> = option_set_names
            .into_iter()
            .map(|name| (name.clone(), name))
            .collect();

        Self::new(
            ListState::new(OptionSetKindEntry::all()),
            AutocompleteState::new(options),
        )
    }

    fn selected_kind(&self) -> Option<OptionSetKindEntry> {
        self.kind_list.with_ref(|s| {
            s.focused_key
                .as_ref()
                .and_then(|key| s.items.iter().find(|item| &item.key() == key))
                .copied()
        })
    }

    fn selected_name(&self) -> Option<String> {
        self.name_autocomplete.with_ref(|s| s.value().cloned())
    }
}

#[modal_impl]
impl OptionSetTypePickerModal {
    fn default_result(&self) -> Option<FieldType> {
        None
    }

    #[on_start]
    async fn on_start(&self, mx: &ModalContext<Option<FieldType>>) {
        mx.focus("kind-list");
    }

    #[keybinds]
    fn keybinds() {
        bind("escape", cancel);
    }

    #[handler]
    async fn cancel(&self, mx: &ModalContext<Option<FieldType>>) {
        mx.close(None);
    }

    #[handler]
    async fn submit(&self, mx: &ModalContext<Option<FieldType>>) {
        let Some(entry) = self.selected_kind() else {
            return;
        };

        // Name is optional — empty means "any option set of this kind"
        let name = self.selected_name().unwrap_or_default();

        mx.close(Some(FieldType::OptionSet {
            kind: entry.kind,
            name,
        }));
    }

    fn element(&self) -> Element {
        let selected_kind = self.selected_kind();
        let selected_name = self.selected_name();

        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                text (content: "Select Option Set Type") style (bold, fg: interact)

                row (width: fill, height: fill, gap: 2) {
                    // Left: kind selector
                    column (width: fill, height: fill, gap: 1) {
                        text (content: "Kind") style (fg: muted)
                        box_ (width: fill, height: fill) {
                            list (state: self.kind_list, id: "kind-list", width: fill, height: fill)
                        }

                        if let Some(entry) = selected_kind {
                            text (content: {entry.description()}) style (fg: muted)
                        }
                    }

                    // Right: option set name picker
                    column (width: fill, height: fill, gap: 1) {
                        text (content: "Option Set Name") style (fg: muted)
                        autocomplete (
                            state: self.name_autocomplete,
                            id: "name-autocomplete",
                            placeholder: "Search option sets...",
                            width: fill
                        )

                        if let Some(ref name) = selected_name {
                            text (content: {format!("Selected: {}", name)}) style (fg: muted)
                        }
                        if selected_name.is_none() {
                            text (content: "No name selected (will match any)") style (fg: muted)
                        }
                    }
                }

                row (width: fill, justify: between) {
                    button (label: "Cancel", hint: "esc", id: "cancel-btn") on_activate: cancel()
                    button (label: "Select", id: "select-btn", disabled: {selected_kind.is_none()}) on_activate: submit()
                }
            }
        }
    }
}
