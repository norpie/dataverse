//! Modal for editing a Convert transform.

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::SelectState;
use tuidom::Element;

/// Target type options for convert transform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ConvertType {
    #[default]
    String,
    Int,
    Decimal,
    Bool,
}

impl ConvertType {
    fn all() -> Vec<(ConvertType, String)> {
        vec![
            (ConvertType::String, "String".to_string()),
            (ConvertType::Int, "Integer".to_string()),
            (ConvertType::Decimal, "Decimal".to_string()),
            (ConvertType::Bool, "Boolean".to_string()),
        ]
    }

    /// Convert from string representation used in TransformData.
    pub fn from_str(s: &str) -> Self {
        match s {
            "int" => ConvertType::Int,
            "decimal" => ConvertType::Decimal,
            "bool" => ConvertType::Bool,
            _ => ConvertType::String,
        }
    }

    /// Convert to string representation for TransformData.
    pub fn as_str(&self) -> &'static str {
        match self {
            ConvertType::String => "string",
            ConvertType::Int => "int",
            ConvertType::Decimal => "decimal",
            ConvertType::Bool => "bool",
        }
    }
}

impl ToString for ConvertType {
    fn to_string(&self) -> String {
        match self {
            ConvertType::String => "String".to_string(),
            ConvertType::Int => "Integer".to_string(),
            ConvertType::Decimal => "Decimal".to_string(),
            ConvertType::Bool => "Boolean".to_string(),
        }
    }
}

/// Modal for editing a Convert transform.
#[modal(size = Sm)]
pub struct ConvertTransformModal {
    /// Type selector.
    type_select: SelectState<ConvertType>,
}

impl ConvertTransformModal {
    /// Create a new Convert transform modal.
    pub fn new_modal(current_type: &str) -> Self {
        let convert_type = ConvertType::from_str(current_type);
        let type_select = SelectState::new(ConvertType::all()).with_value(convert_type);
        Self::new(type_select)
    }
}

#[modal_impl]
impl ConvertTransformModal {
    fn default_result(&self) -> Option<String> {
        None
    }

    #[on_start]
    async fn on_start(&self, mx: &ModalContext<Option<String>>) {
        mx.focus("type-select");
    }

    #[keybinds]
    fn keybinds() {
        bind("escape", cancel);
        bind("ctrl+s", save);
    }

    #[handler]
    async fn cancel(&self, mx: &ModalContext<Option<String>>) {
        mx.close(None);
    }

    #[handler]
    async fn save(&self, mx: &ModalContext<Option<String>>) {
        if let Some(selected) = self.type_select.get().value() {
            mx.close(Some(selected.as_str().to_string()));
        }
    }

    fn element(&self) -> Element {
        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                text (content: "Edit Convert Transform") style (bold, fg: interact)

                column (gap: 0, width: fill) {
                    text (content: "Convert to") style (fg: muted)
                    select (state: self.type_select, id: "type-select", width: fill)
                }

                // Help text
                text (content: "Converts the current value to the selected type.") style (fg: muted)

                // Spacer
                box_ (height: fill) {}

                // Buttons
                row (width: fill, justify: between) {
                    button (label: "Cancel", hint: "esc", id: "cancel-btn")
                        on_activate: cancel()
                    button (label: "Save", hint: "ctrl+s", id: "save-btn")
                        on_activate: save()
                }
            }
        }
    }
}
