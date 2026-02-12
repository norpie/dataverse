//! Modal for editing a StringOps transform.

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::Button;
use rafter::widgets::NumberInput;
use rafter::widgets::NumberInputState;
use rafter::widgets::Select;
use rafter::widgets::SelectState;
use rafter::widgets::Text;
use tuidom::Element;

use crate::apps::migration::types::StringOp;

impl StringOp {
    fn label(&self) -> String {
        match self {
            StringOp::Uppercase => "Uppercase".to_string(),
            StringOp::Lowercase => "Lowercase".to_string(),
            StringOp::Trim => "Trim (both ends)".to_string(),
            StringOp::TrimStart => "Trim start".to_string(),
            StringOp::TrimEnd => "Trim end".to_string(),
            StringOp::Truncate(max) => format!("Truncate ({})", max),
        }
    }

    fn all() -> Vec<(StringOp, String)> {
        vec![
            (StringOp::Uppercase, "Uppercase".to_string()),
            (StringOp::Lowercase, "Lowercase".to_string()),
            (StringOp::Trim, "Trim (both ends)".to_string()),
            (StringOp::TrimStart, "Trim start".to_string()),
            (StringOp::TrimEnd, "Trim end".to_string()),
            (StringOp::Truncate(0), "Truncate".to_string()),
        ]
    }

    /// Returns true if this operation requires a length parameter.
    fn needs_length(&self) -> bool {
        matches!(self, StringOp::Truncate(_))
    }
}

impl ToString for StringOp {
    fn to_string(&self) -> String {
        self.label()
    }
}

/// Modal for editing a StringOps transform.
#[modal(size = Sm)]
pub struct StringOpsTransformModal {
    /// Operation selector.
    op_select: SelectState<StringOp>,
    /// Max length input for Truncate.
    max_length: NumberInputState,
}

impl StringOpsTransformModal {
    /// Create a new StringOps transform modal with the given initial operation.
    pub fn new_modal(current_op: StringOp) -> Self {
        let initial_length = match &current_op {
            StringOp::Truncate(n) => *n as f64,
            _ => 100.0,
        };
        let op_select = SelectState::new(StringOp::all()).with_value(current_op);
        let max_length = NumberInputState::new(initial_length)
            .with_min(1.0)
            .with_step(1.0);
        Self::new(op_select, max_length)
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
            let result = match op {
                StringOp::Truncate(_) => {
                    StringOp::Truncate(self.max_length.get().value() as usize)
                }
                other => other,
            };
            mx.close(Some(result));
        }
    }

    fn element(&self) -> Element {
        let show_length = self
            .op_select
            .get()
            .value()
            .map(|op| op.needs_length())
            .unwrap_or(false);

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

                // Max length input (only for Truncate)
                if show_length {
                    column (gap: 0, width: fill) {
                        text (content: "Max length") style (fg: muted)
                        number_input (
                            state: self.max_length,
                            id: "max-length"
                        )
                    }
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
