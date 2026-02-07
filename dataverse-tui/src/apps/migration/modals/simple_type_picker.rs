//! Modal for selecting a simple (scalar) AttributeType.

use dataverse_lib::model::metadata::AttributeType;
use dataverse_lib::model::FieldType;
use rafter::page;
use rafter::prelude::*;
use rafter::widgets::Button;
use rafter::widgets::List;
use rafter::widgets::ListItem;
use rafter::widgets::ListState;
use rafter::widgets::Text;
use tuidom::Element;

/// A simple type entry for the picker list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SimpleTypeEntry {
    attr: AttributeType,
}

impl SimpleTypeEntry {
    fn new(attr: AttributeType) -> Self {
        Self { attr }
    }

    fn label(&self) -> &'static str {
        match self.attr {
            AttributeType::String => "String",
            AttributeType::Memo => "Memo (Multi-line Text)",
            AttributeType::Integer => "Integer",
            AttributeType::BigInt => "Big Integer",
            AttributeType::Decimal => "Decimal",
            AttributeType::Double => "Double",
            AttributeType::Boolean => "Boolean",
            AttributeType::DateTime => "Date Time",
            AttributeType::Money => "Money",
            AttributeType::Uniqueidentifier => "Unique Identifier (GUID)",
            _ => "Unknown",
        }
    }

    fn description(&self) -> &'static str {
        match self.attr {
            AttributeType::String => "Single-line text value",
            AttributeType::Memo => "Multi-line text value",
            AttributeType::Integer => "32-bit whole number",
            AttributeType::BigInt => "64-bit whole number",
            AttributeType::Decimal => "Fixed-precision decimal number",
            AttributeType::Double => "Floating-point number",
            AttributeType::Boolean => "True or false value",
            AttributeType::DateTime => "Date and time value",
            AttributeType::Money => "Currency value",
            AttributeType::Uniqueidentifier => "Globally unique identifier (GUID)",
            _ => "",
        }
    }

    fn all() -> Vec<Self> {
        vec![
            Self::new(AttributeType::String),
            Self::new(AttributeType::Memo),
            Self::new(AttributeType::Integer),
            Self::new(AttributeType::BigInt),
            Self::new(AttributeType::Decimal),
            Self::new(AttributeType::Double),
            Self::new(AttributeType::Boolean),
            Self::new(AttributeType::DateTime),
            Self::new(AttributeType::Money),
            Self::new(AttributeType::Uniqueidentifier),
        ]
    }
}

impl ListItem for SimpleTypeEntry {
    type Key = String;

    fn key(&self) -> String {
        self.label().to_string()
    }

    fn render(&self) -> Element {
        Element::text(self.label())
    }
}

/// Modal for picking a simple field type.
///
/// Returns `Some(FieldType::Simple(attr))` on selection, `None` on cancel.
#[modal(size = Sm)]
pub struct SimpleTypePickerModal {
    list_state: ListState<SimpleTypeEntry>,
}

impl SimpleTypePickerModal {
    /// Create a new simple type picker modal.
    pub fn new_modal() -> Self {
        Self::new(ListState::new(SimpleTypeEntry::all()))
    }

    fn selected_entry(&self) -> Option<SimpleTypeEntry> {
        self.list_state.with_ref(|s| {
            s.focused_key
                .as_ref()
                .and_then(|key| s.items.iter().find(|item| &item.key() == key))
                .copied()
        })
    }
}

#[modal_impl]
impl SimpleTypePickerModal {
    fn default_result(&self) -> Option<FieldType> {
        None
    }

    #[on_start]
    async fn on_start(&self, mx: &ModalContext<Option<FieldType>>) {
        mx.focus("simple-type-list");
    }

    #[keybinds]
    fn keybinds() {
        bind("escape", cancel);
        bind("enter", submit);
    }

    #[handler]
    async fn cancel(&self, mx: &ModalContext<Option<FieldType>>) {
        mx.close(None);
    }

    #[handler]
    async fn submit(&self, mx: &ModalContext<Option<FieldType>>) {
        if let Some(entry) = self.selected_entry() {
            mx.close(Some(FieldType::Simple(entry.attr)));
        }
    }

    fn element(&self) -> Element {
        let selected = self.selected_entry();

        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                text (content: "Select Simple Type") style (bold, fg: interact)

                row (width: fill, height: fill, gap: 2) {
                    box_ (width: fill, height: fill) {
                        list (state: self.list_state, id: "simple-type-list", width: fill, height: fill)
                            on_activate: submit()
                    }

                    column (width: fill, height: fill) {
                        if let Some(entry) = selected {
                            column (gap: 1) {
                                text (content: {entry.label()}) style (bold, fg: interact)
                                text (content: {entry.description()}) style (fg: muted)
                            }
                        } else {
                            text (content: "Select a type") style (fg: muted)
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
