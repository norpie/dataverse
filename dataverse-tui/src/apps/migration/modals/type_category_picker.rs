//! Modal for selecting a type category (Simple, Lookup, or OptionSet).

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::Button;
use rafter::widgets::List;
use rafter::widgets::ListItem;
use rafter::widgets::ListState;
use rafter::widgets::Text;
use tuidom::Element;
use tuidom::Style;

/// The three categories of field types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeCategory {
    /// Simple scalar types (String, Integer, Boolean, etc.)
    Simple,
    /// Lookup types (Lookup, Customer, Owner) with target entities.
    Lookup,
    /// Option set types (Picklist, State, Status, MultiSelectPicklist).
    OptionSet,
}

impl TypeCategory {
    fn label(&self) -> &'static str {
        match self {
            TypeCategory::Simple => "Simple",
            TypeCategory::Lookup => "Lookup",
            TypeCategory::OptionSet => "Option Set",
        }
    }

    fn description(&self) -> &'static str {
        match self {
            TypeCategory::Simple => "Scalar types like String, Integer, Boolean, DateTime, etc.",
            TypeCategory::Lookup => "Reference to another entity (Lookup, Customer, Owner)",
            TypeCategory::OptionSet => {
                "Choice field (Picklist, State, Status, Multi-Select Picklist)"
            }
        }
    }

    fn all() -> Vec<TypeCategory> {
        vec![
            TypeCategory::Simple,
            TypeCategory::Lookup,
            TypeCategory::OptionSet,
        ]
    }
}

impl ListItem for TypeCategory {
    type Key = &'static str;

    fn key(&self) -> &'static str {
        self.label()
    }

    fn render(&self) -> Element {
        Element::text(self.label())
    }
}

/// Modal for picking a type category.
#[modal(size = Sm)]
pub struct TypeCategoryPickerModal {
    list_state: ListState<TypeCategory>,
}

impl TypeCategoryPickerModal {
    /// Create a new type category picker modal.
    pub fn new_modal() -> Self {
        Self::new(ListState::new(TypeCategory::all()))
    }

    fn selected_category(&self) -> Option<TypeCategory> {
        self.list_state.with_ref(|s| {
            s.focused_key
                .as_ref()
                .and_then(|key| s.items.iter().find(|item| &item.key() == key))
                .copied()
        })
    }
}

#[modal_impl]
impl TypeCategoryPickerModal {
    fn default_result(&self) -> Option<TypeCategory> {
        None
    }

    #[on_start]
    async fn on_start(&self, mx: &ModalContext<Option<TypeCategory>>) {
        mx.focus("category-list");
    }

    #[keybinds]
    fn keybinds() {
        bind("escape", cancel);
        bind("enter", submit);
    }

    #[handler]
    async fn cancel(&self, mx: &ModalContext<Option<TypeCategory>>) {
        mx.close(None);
    }

    #[handler]
    async fn submit(&self, mx: &ModalContext<Option<TypeCategory>>) {
        if let Some(category) = self.selected_category() {
            mx.close(Some(category));
        }
    }

    fn element(&self) -> Element {
        let selected = self.selected_category();

        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                text (content: "Select Type Category") style (bold, fg: interact)

                row (width: fill, height: fill, gap: 2) {
                    box_ (width: fill, height: fill) {
                        list (state: self.list_state, id: "category-list", width: fill, height: fill)
                            on_activate: submit()
                    }

                    column (width: fill, height: fill) {
                        if let Some(cat) = selected {
                            column (gap: 1) {
                                text (content: {cat.label()}) style (bold, fg: interact)
                                text (content: {cat.description()}) style (fg: muted)
                            }
                        } else {
                            text (content: "Select a category") style (fg: muted)
                        }
                    }
                }

                row (width: fill, justify: between) {
                    button (label: "Cancel", hint: "esc", id: "cancel-btn") on_activate: cancel()
                    button (label: "Select", hint: "enter", id: "select-btn", disabled: {selected.is_none()}) on_activate: submit()
                }
            }
        }
    }
}
