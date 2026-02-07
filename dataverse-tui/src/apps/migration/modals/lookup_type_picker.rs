//! Modal for picking a Lookup field type (kind + target entities).

use dataverse_lib::model::metadata::AttributeType;
use dataverse_lib::model::FieldType;
use rafter::page;
use rafter::prelude::*;
use rafter::widgets::Autocomplete;
use rafter::widgets::AutocompleteState;
use rafter::widgets::Button;
use rafter::widgets::List;
use rafter::widgets::ListItem;
use rafter::widgets::ListState;
use rafter::widgets::SelectionMode;
use rafter::widgets::Text;
use tuidom::Element;

/// Lookup kind entry for the kind selector list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LookupKindEntry {
    kind: AttributeType,
}

impl LookupKindEntry {
    fn label(&self) -> &'static str {
        match self.kind {
            AttributeType::Lookup => "Lookup",
            AttributeType::Customer => "Customer",
            AttributeType::Owner => "Owner",
            _ => "Unknown",
        }
    }

    fn description(&self) -> &'static str {
        match self.kind {
            AttributeType::Lookup => "Reference to a single entity type",
            AttributeType::Customer => "Reference to account or contact",
            AttributeType::Owner => "Reference to user or team",
            _ => "",
        }
    }

    fn all() -> Vec<Self> {
        vec![
            Self {
                kind: AttributeType::Lookup,
            },
            Self {
                kind: AttributeType::Customer,
            },
            Self {
                kind: AttributeType::Owner,
            },
        ]
    }
}

impl ListItem for LookupKindEntry {
    type Key = String;

    fn key(&self) -> String {
        self.label().to_string()
    }

    fn render(&self) -> Element {
        Element::text(self.label())
    }
}

/// Modal for picking a lookup field type.
///
/// The caller provides a list of available entity names (pre-fetched).
/// Returns `Some(FieldType::Lookup { kind, targets })` on confirmation, `None` on cancel.
#[modal(size = Md)]
pub struct LookupTypePickerModal {
    kind_list: ListState<LookupKindEntry>,
    entity_autocomplete: AutocompleteState<String>,
}

impl LookupTypePickerModal {
    /// Create a new lookup type picker modal.
    ///
    /// `entities` is the pre-fetched list of entity logical names.
    pub fn new_modal(entities: Vec<String>) -> Self {
        let options: Vec<(String, String)> = entities
            .into_iter()
            .map(|name| (name.clone(), name))
            .collect();
        let autocomplete =
            AutocompleteState::new(options).with_selection(SelectionMode::Multi);

        Self::new(ListState::new(LookupKindEntry::all()), autocomplete)
    }

    fn selected_kind(&self) -> Option<LookupKindEntry> {
        self.kind_list.with_ref(|s| {
            s.focused_key
                .as_ref()
                .and_then(|key| s.items.iter().find(|item| &item.key() == key))
                .copied()
        })
    }

    fn selected_targets(&self) -> Vec<String> {
        self.entity_autocomplete
            .with_ref(|s| s.selected_values().cloned().collect())
    }
}

#[modal_impl]
impl LookupTypePickerModal {
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

        let targets = self.selected_targets();
        mx.close(Some(FieldType::Lookup {
            kind: entry.kind,
            targets,
        }));
    }

    fn element(&self) -> Element {
        let selected_kind = self.selected_kind();
        let targets = self.selected_targets();
        let target_summary = if targets.is_empty() {
            "Any entity".to_string()
        } else {
            targets.join(", ")
        };

        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                text (content: "Select Lookup Type") style (bold, fg: interact)

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

                    // Right: entity target picker
                    column (width: fill, height: fill, gap: 1) {
                        text (content: "Target Entities") style (fg: muted)
                        autocomplete (
                            state: self.entity_autocomplete,
                            id: "entity-autocomplete",
                            placeholder: "Search entities...",
                            width: fill
                        )
                        text (content: {format!("Selected: {}", target_summary)}) style (fg: muted)
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
