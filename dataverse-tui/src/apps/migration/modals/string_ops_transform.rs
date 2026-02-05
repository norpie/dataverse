//! Modal for editing a StringOps transform.

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::Button;
use rafter::widgets::Select;
use rafter::widgets::SelectState;
use rafter::widgets::Text;
use tuidom::Element;

use crate::apps::migration::types::StringOp;

impl StringOp {
    fn label(&self) -> &'static str {
        match self {
            StringOp::Uppercase => "Uppercase",
            StringOp::Lowercase => "Lowercase",
            StringOp::Trim => "Trim (both ends)",
            StringOp::TrimStart => "Trim start",
            StringOp::TrimEnd => "Trim end",
        }
    }

    fn all() -> Vec<(StringOp, String)> {
        vec![
            (StringOp::Uppercase, "Uppercase".to_string()),
            (StringOp::Lowercase, "Lowercase".to_string()),
            (StringOp::Trim, "Trim (both ends)".to_string()),
            (StringOp::TrimStart, "Trim start".to_string()),
            (StringOp::TrimEnd, "Trim end".to_string()),
        ]
    }
}

impl ToString for StringOp {
    fn to_string(&self) -> String {
        self.label().to_string()
    }
}

/// Modal for editing a StringOps transform.
#[modal(size = Sm)]
pub struct StringOpsTransformModal {
    /// Operation selector.
    op_select: SelectState<StringOp>,
}

impl StringOpsTransformModal {
    /// Create a new StringOps transform modal with the given initial operation.
    pub fn new_modal(current_op: StringOp) -> Self {
        let op_select = SelectState::new(StringOp::all()).with_value(current_op);
        Self::new(op_select)
    }
}

#[modal_impl]
impl StringOpsTransformModal {
    fn default_result(&self) -> Option<StringOp> {
        None
    }

    #[on_start]
    async fn on_start(&self, mx: &ModalContext<Option<StringOp>>) {
        mx.focus("op-select");
    }

    #[keybinds]
    fn keybinds() {
        bind("escape", cancel);
        bind("ctrl+s", save);
    }

    #[handler]
    async fn cancel(&self, mx: &ModalContext<Option<StringOp>>) {
        mx.close(None);
    }

    #[handler]
    async fn save(&self, mx: &ModalContext<Option<StringOp>>) {
        if let Some(op) = self.op_select.get().value().cloned() {
            mx.close(Some(op));
        }
    }

    fn element(&self) -> Element {
        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                text (content: "Edit String Operation") style (bold, fg: interact)

                // Operation selector
                column (gap: 0, width: fill) {
                    text (content: "Operation") style (fg: muted)
                    select (
                        state: self.op_select,
                        id: "op-select",
                        width: fill
                    )
                }

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
