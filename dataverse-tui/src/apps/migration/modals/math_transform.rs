//! Modal for editing a Math transform.

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::Button;
use rafter::widgets::NumberInput;
use rafter::widgets::NumberInputState;
use rafter::widgets::Select;
use rafter::widgets::SelectState;
use rafter::widgets::Text;
use tuidom::Element;

use crate::apps::migration::types::MathOp;

/// The arithmetic operation kind (without operand).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
enum MathOpKind {
    #[default]
    Add,
    Subtract,
    Multiply,
    Divide,
    Round,
}

impl MathOpKind {
    fn label(&self) -> &'static str {
        match self {
            MathOpKind::Add => "Add",
            MathOpKind::Subtract => "Subtract",
            MathOpKind::Multiply => "Multiply",
            MathOpKind::Divide => "Divide",
            MathOpKind::Round => "Round",
        }
    }

    fn all() -> Vec<(MathOpKind, String)> {
        vec![
            (MathOpKind::Add, "Add".to_string()),
            (MathOpKind::Subtract, "Subtract".to_string()),
            (MathOpKind::Multiply, "Multiply".to_string()),
            (MathOpKind::Divide, "Divide".to_string()),
            (MathOpKind::Round, "Round".to_string()),
        ]
    }

    fn operand_label(&self) -> &'static str {
        match self {
            MathOpKind::Round => "Decimal Places",
            _ => "Value",
        }
    }
}

impl ToString for MathOpKind {
    fn to_string(&self) -> String {
        self.label().to_string()
    }
}

/// Modal for editing a Math transform.
#[modal(size = Sm)]
pub struct MathTransformModal {
    /// Operation selector.
    op_select: SelectState<MathOpKind>,
    /// Operand number input (f64 for arithmetic, integer for Round).
    operand: NumberInputState,
    /// Validation error message.
    error: Option<String>,
}

impl MathTransformModal {
    /// Create a new Math transform modal with the given initial operation.
    pub fn new_modal(current_op: MathOp) -> Self {
        let (kind, operand) = Self::decompose_op(&current_op);
        let op_select = SelectState::new(MathOpKind::all()).with_value(kind);
        Self::new(op_select, operand, None)
    }

    fn decompose_op(op: &MathOp) -> (MathOpKind, NumberInputState) {
        match op {
            MathOp::Add(n) => (MathOpKind::Add, NumberInputState::new(*n).allow_negative()),
            MathOp::Subtract(n) => (
                MathOpKind::Subtract,
                NumberInputState::new(*n).allow_negative(),
            ),
            MathOp::Multiply(n) => (
                MathOpKind::Multiply,
                NumberInputState::new(*n).allow_negative(),
            ),
            MathOp::Divide(n) => (
                MathOpKind::Divide,
                NumberInputState::new(*n).allow_negative(),
            ),
            MathOp::Round(places) => (
                MathOpKind::Round,
                NumberInputState::new(*places as f64)
                    .integer()
                    .with_min(0.0)
                    .with_max(10.0),
            ),
        }
    }

    fn selected_kind(&self) -> MathOpKind {
        self.op_select.get().value().copied().unwrap_or_default()
    }

    fn build_op(&self) -> Result<MathOp, String> {
        let kind = self.selected_kind();
        let value = self.operand.get().value();
        match kind {
            MathOpKind::Add => Ok(MathOp::Add(value)),
            MathOpKind::Subtract => Ok(MathOp::Subtract(value)),
            MathOpKind::Multiply => Ok(MathOp::Multiply(value)),
            MathOpKind::Divide => {
                if value == 0.0 {
                    Err("Cannot divide by zero".to_string())
                } else {
                    Ok(MathOp::Divide(value))
                }
            }
            MathOpKind::Round => Ok(MathOp::Round(value as u32)),
        }
    }
}

#[modal_impl]
impl MathTransformModal {
    fn default_result(&self) -> Option<MathOp> {
        None
    }

    #[on_start]
    async fn on_start(&self, mx: &ModalContext<Option<MathOp>>) {
        mx.focus("op-select");
    }

    #[keybinds]
    fn keybinds() {
        bind("escape", cancel);
        bind("ctrl+s", save);
    }

    #[handler]
    async fn cancel(&self, mx: &ModalContext<Option<MathOp>>) {
        mx.close(None);
    }

    #[handler]
    async fn save(&self, mx: &ModalContext<Option<MathOp>>) {
        match self.build_op() {
            Ok(op) => mx.close(Some(op)),
            Err(e) => self.error.set(Some(e)),
        }
    }

    #[handler]
    async fn on_op_change(&self, mx: &ModalContext<Option<MathOp>>) {
        self.error.set(None);
        let kind = self.selected_kind();
        // Reset operand to appropriate defaults when switching operations
        match kind {
            MathOpKind::Round => {
                self.operand.set(
                    NumberInputState::new(0.0)
                        .integer()
                        .with_min(0.0)
                        .with_max(10.0),
                );
            }
            _ => {
                self.operand
                    .set(NumberInputState::new(0.0).allow_negative());
            }
        }
        mx.focus("operand-input");
    }

    #[handler]
    async fn on_operand_change(&self, _mx: &ModalContext<Option<MathOp>>) {
        self.error.set(None);
    }

    fn element(&self) -> Element {
        let kind = self.selected_kind();
        let error = self.error.get();

        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                text (content: "Edit Math Transform") style (bold, fg: interact)

                // Operation selector
                column (gap: 0, width: fill) {
                    text (content: "Operation") style (fg: muted)
                    select (
                        state: self.op_select,
                        id: "op-select",
                        width: fill
                    )
                        on_change: on_op_change()
                }

                // Operand input
                column (gap: 0, width: fill) {
                    text (content: {kind.operand_label()}) style (fg: muted)
                    number_input (
                        state: self.operand,
                        id: "operand-input",
                        placeholder: "0",
                        width: fill
                    )
                        on_change: on_operand_change()
                }

                // Error message
                if let Some(err) = error {
                    text (content: {&err}) style (fg: error)
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
