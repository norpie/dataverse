//! Modal for editing a Guard transform condition.

use dataverse_lib::model::Value;
use dataverse_lib::DataverseClient;
use rafter::page;
use rafter::prelude::*;
use rafter::widgets::AutocompleteState;
use rafter::widgets::Button;
use rafter::widgets::Input;
use rafter::widgets::NumberInput;
use rafter::widgets::NumberInputState;
use rafter::widgets::Select;
use rafter::widgets::SelectState;
use rafter::widgets::Text;
use tuidom::Element;

use super::path_suggestions::PathSuggestionGenerator;
use super::path_suggestions::VariableInfo;
use crate::apps::migration::types::CompareOp;
use crate::apps::migration::types::Condition;
use crate::apps::migration::types::Expr;
use crate::apps::migration::types::SystemVar;

/// Guard operator kind — determines which right-side input to show.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
enum GuardOp {
    // Comparison (number input)
    #[default]
    Equal,
    NotEqual,
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,
    // Null checks (no right side)
    IsNull,
    IsNotNull,
    // String checks (text input)
    Contains,
    StartsWith,
    EndsWith,
}

impl GuardOp {
    fn label(&self) -> &'static str {
        match self {
            GuardOp::Equal => "==",
            GuardOp::NotEqual => "!=",
            GuardOp::LessThan => "<",
            GuardOp::LessThanOrEqual => "<=",
            GuardOp::GreaterThan => ">",
            GuardOp::GreaterThanOrEqual => ">=",
            GuardOp::IsNull => "is null",
            GuardOp::IsNotNull => "is not null",
            GuardOp::Contains => "contains",
            GuardOp::StartsWith => "starts with",
            GuardOp::EndsWith => "ends with",
        }
    }

    fn all() -> Vec<(GuardOp, String)> {
        vec![
            (GuardOp::Equal, "==".to_string()),
            (GuardOp::NotEqual, "!=".to_string()),
            (GuardOp::LessThan, "<".to_string()),
            (GuardOp::LessThanOrEqual, "<=".to_string()),
            (GuardOp::GreaterThan, ">".to_string()),
            (GuardOp::GreaterThanOrEqual, ">=".to_string()),
            (GuardOp::IsNull, "is null".to_string()),
            (GuardOp::IsNotNull, "is not null".to_string()),
            (GuardOp::Contains, "contains".to_string()),
            (GuardOp::StartsWith, "starts with".to_string()),
            (GuardOp::EndsWith, "ends with".to_string()),
        ]
    }

    fn needs_number(&self) -> bool {
        matches!(
            self,
            GuardOp::Equal
                | GuardOp::NotEqual
                | GuardOp::LessThan
                | GuardOp::LessThanOrEqual
                | GuardOp::GreaterThan
                | GuardOp::GreaterThanOrEqual
        )
    }

    fn needs_string(&self) -> bool {
        matches!(
            self,
            GuardOp::Contains | GuardOp::StartsWith | GuardOp::EndsWith
        )
    }
}

impl ToString for GuardOp {
    fn to_string(&self) -> String {
        self.label().to_string()
    }
}

/// Modal for editing a Guard transform condition.
#[modal(size = Sm)]
pub struct GuardTransformModal {
    /// The Dataverse client for metadata lookups.
    #[state(skip)]
    client: DataverseClient,
    /// The source entity logical name.
    #[state(skip)]
    source_entity: String,
    /// Available variables with type info.
    #[state(skip)]
    variables: Vec<VariableInfo>,

    /// Left-side expression (autocomplete with path traversal).
    left: AutocompleteState<String>,
    /// Operator selector.
    op_select: SelectState<GuardOp>,
    /// Right-side number value (for comparison ops).
    number_value: NumberInputState,
    /// Right-side string value (for string ops).
    string_value: String,
    /// Validation error message.
    error: Option<String>,
}

impl GuardTransformModal {
    /// Create a new Guard transform modal with the given initial condition.
    pub fn new_modal(
        client: DataverseClient,
        source_entity: String,
        variables: Vec<VariableInfo>,
        condition: Condition,
    ) -> Self {
        let (left_text, op, number_val, string_val) = Self::decompose_condition(&condition);

        let mut left = AutocompleteState::new(Vec::<(String, String)>::new());
        left.text = left_text;
        left.cursor = left.text.len();

        let op_select = SelectState::new(GuardOp::all()).with_value(op);

        let number_value = NumberInputState::new(number_val).allow_negative();

        Self::new(
            client,
            source_entity,
            variables,
            left,
            op_select,
            number_value,
            string_val,
            None,
        )
    }

    fn decompose_condition(condition: &Condition) -> (String, GuardOp, f64, String) {
        match condition {
            Condition::IsNull(expr) => (expr_to_text(expr), GuardOp::IsNull, 0.0, String::new()),
            Condition::IsNotNull(expr) => {
                (expr_to_text(expr), GuardOp::IsNotNull, 0.0, String::new())
            }
            Condition::Compare { left, op, right } => {
                let guard_op = match op {
                    CompareOp::Equal => GuardOp::Equal,
                    CompareOp::NotEqual => GuardOp::NotEqual,
                    CompareOp::LessThan => GuardOp::LessThan,
                    CompareOp::LessThanOrEqual => GuardOp::LessThanOrEqual,
                    CompareOp::GreaterThan => GuardOp::GreaterThan,
                    CompareOp::GreaterThanOrEqual => GuardOp::GreaterThanOrEqual,
                };
                let num = literal_to_f64(right);
                (expr_to_text(left), guard_op, num, String::new())
            }
            Condition::Contains { value, substring } => (
                expr_to_text(value),
                GuardOp::Contains,
                0.0,
                literal_to_string(substring),
            ),
            Condition::StartsWith { value, prefix } => (
                expr_to_text(value),
                GuardOp::StartsWith,
                0.0,
                literal_to_string(prefix),
            ),
            Condition::EndsWith { value, suffix } => (
                expr_to_text(value),
                GuardOp::EndsWith,
                0.0,
                literal_to_string(suffix),
            ),
            // And/Or/Not shouldn't appear in a single guard, but handle gracefully
            _ => (String::new(), GuardOp::IsNull, 0.0, String::new()),
        }
    }

    fn selected_op(&self) -> GuardOp {
        self.op_select.get().value().copied().unwrap_or_default()
    }

    fn left_text(&self) -> String {
        self.left.get().text.clone()
    }

    fn build_condition(&self) -> Result<Condition, String> {
        let left_text = self.left_text();
        if left_text.trim().is_empty() {
            return Err("Expression cannot be empty".to_string());
        }
        let left_expr = parse_expr(&left_text);
        let op = self.selected_op();

        match op {
            GuardOp::IsNull => Ok(Condition::IsNull(left_expr)),
            GuardOp::IsNotNull => Ok(Condition::IsNotNull(left_expr)),
            GuardOp::Equal => Ok(Condition::Compare {
                left: left_expr,
                op: CompareOp::Equal,
                right: Expr::Literal(Value::Float(self.number_value.get().value())),
            }),
            GuardOp::NotEqual => Ok(Condition::Compare {
                left: left_expr,
                op: CompareOp::NotEqual,
                right: Expr::Literal(Value::Float(self.number_value.get().value())),
            }),
            GuardOp::LessThan => Ok(Condition::Compare {
                left: left_expr,
                op: CompareOp::LessThan,
                right: Expr::Literal(Value::Float(self.number_value.get().value())),
            }),
            GuardOp::LessThanOrEqual => Ok(Condition::Compare {
                left: left_expr,
                op: CompareOp::LessThanOrEqual,
                right: Expr::Literal(Value::Float(self.number_value.get().value())),
            }),
            GuardOp::GreaterThan => Ok(Condition::Compare {
                left: left_expr,
                op: CompareOp::GreaterThan,
                right: Expr::Literal(Value::Float(self.number_value.get().value())),
            }),
            GuardOp::GreaterThanOrEqual => Ok(Condition::Compare {
                left: left_expr,
                op: CompareOp::GreaterThanOrEqual,
                right: Expr::Literal(Value::Float(self.number_value.get().value())),
            }),
            GuardOp::Contains => {
                let s = self.string_value.get().clone();
                if s.is_empty() {
                    return Err("Substring cannot be empty".to_string());
                }
                Ok(Condition::Contains {
                    value: left_expr,
                    substring: Expr::Literal(Value::String(s)),
                })
            }
            GuardOp::StartsWith => {
                let s = self.string_value.get().clone();
                if s.is_empty() {
                    return Err("Prefix cannot be empty".to_string());
                }
                Ok(Condition::StartsWith {
                    value: left_expr,
                    prefix: Expr::Literal(Value::String(s)),
                })
            }
            GuardOp::EndsWith => {
                let s = self.string_value.get().clone();
                if s.is_empty() {
                    return Err("Suffix cannot be empty".to_string());
                }
                Ok(Condition::EndsWith {
                    value: left_expr,
                    suffix: Expr::Literal(Value::String(s)),
                })
            }
        }
    }
}

#[modal_impl]
impl GuardTransformModal {
    fn default_result(&self) -> Option<Condition> {
        None
    }

    #[on_start]
    async fn on_start(&self, mx: &ModalContext<Option<Condition>>) {
        // Generate initial suggestions for left side
        let path = self.left_text();
        let generator = PathSuggestionGenerator::new(
            self.client.clone(),
            self.source_entity.clone(),
            self.variables.clone(),
        );
        let suggestions = generator.generate_suggestions(&path).await;

        self.left.update(|s| {
            s.options = suggestions;
            s.refilter();
        });

        mx.focus("left-autocomplete");
    }

    #[keybinds]
    fn keybinds() {
        bind("escape", cancel);
        bind("ctrl+s", save);
    }

    #[handler]
    async fn cancel(&self, mx: &ModalContext<Option<Condition>>) {
        mx.close(None);
    }

    #[handler]
    async fn save(&self, mx: &ModalContext<Option<Condition>>) {
        match self.build_condition() {
            Ok(condition) => mx.close(Some(condition)),
            Err(e) => self.error.set(Some(e)),
        }
    }

    #[handler]
    async fn on_left_change(&self, _cx: &AppContext) {
        self.error.set(None);
        let path = self.left_text();

        let generator = PathSuggestionGenerator::new(
            self.client.clone(),
            self.source_entity.clone(),
            self.variables.clone(),
        );
        let suggestions = generator.generate_suggestions(&path).await;

        self.left.update(|s| {
            s.options = suggestions;
            s.refilter();
        });
    }

    #[handler]
    async fn on_op_change(&self, mx: &ModalContext<Option<Condition>>) {
        self.error.set(None);
        let op = self.selected_op();
        if op.needs_number() {
            mx.focus("number-input");
        } else if op.needs_string() {
            mx.focus("string-input");
        }
    }

    #[handler]
    async fn on_value_change(&self, _mx: &ModalContext<Option<Condition>>) {
        self.error.set(None);
    }

    fn element(&self) -> Element {
        let op = self.selected_op();
        let error = self.error.get();

        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                text (content: "Edit Guard Condition") style (bold, fg: interact)

                // Left side expression (autocomplete)
                column (gap: 0, width: fill) {
                    text (content: "Expression") style (fg: muted)
                    autocomplete (
                        state: self.left,
                        id: "left-autocomplete",
                        placeholder: "e.g., #value, $variable, fieldname",
                        width: fill
                    )
                        on_change: on_left_change()
                }

                // Operator
                column (gap: 0, width: fill) {
                    text (content: "Operator") style (fg: muted)
                    select (
                        state: self.op_select,
                        id: "op-select",
                        width: fill
                    )
                        on_change: on_op_change()
                }

                // Right side (conditional)
                if op.needs_number() {
                    column (gap: 0, width: fill) {
                        text (content: "Value") style (fg: muted)
                        number_input (
                            state: self.number_value,
                            id: "number-input",
                            placeholder: "0",
                            width: fill
                        )
                            on_change: on_value_change()
                    }
                }
                if op.needs_string() {
                    column (gap: 0, width: fill) {
                        text (content: "Value") style (fg: muted)
                        input (
                            state: self.string_value,
                            id: "string-input",
                            placeholder: "Enter text...",
                            width: fill
                        )
                            on_change: on_value_change()
                    }
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

// =============================================================================
// Helpers
// =============================================================================

/// Convert an Expr to display text for the autocomplete field.
fn expr_to_text(expr: &Expr) -> String {
    match expr {
        Expr::Path(p) => p.clone(),
        Expr::Variable(v) => format!("${}", v),
        Expr::SystemVar(sv) => match sv {
            SystemVar::Value => "#value".to_string(),
            SystemVar::Type => "#type".to_string(),
            SystemVar::Index => "#index".to_string(),
            SystemVar::SourceEntity => "#source_entity".to_string(),
            SystemVar::TargetEntity => "#target_entity".to_string(),
        },
        Expr::Literal(v) => format!("{:?}", v),
    }
}

/// Parse text into an Expr.
fn parse_expr(text: &str) -> Expr {
    let trimmed = text.trim();
    if let Some(name) = trimmed.strip_prefix('#') {
        match name {
            "value" => Expr::SystemVar(SystemVar::Value),
            "type" => Expr::SystemVar(SystemVar::Type),
            "index" => Expr::SystemVar(SystemVar::Index),
            "source_entity" => Expr::SystemVar(SystemVar::SourceEntity),
            "target_entity" => Expr::SystemVar(SystemVar::TargetEntity),
            _ => Expr::Path(trimmed.to_string()),
        }
    } else if let Some(name) = trimmed.strip_prefix('$') {
        Expr::Variable(name.to_string())
    } else {
        Expr::Path(trimmed.to_string())
    }
}

/// Extract f64 from a literal Expr, defaulting to 0.0.
fn literal_to_f64(expr: &Expr) -> f64 {
    match expr {
        Expr::Literal(Value::Float(n)) => *n,
        Expr::Literal(Value::Int(n)) => *n as f64,
        Expr::Literal(Value::Long(n)) => *n as f64,
        Expr::Literal(Value::Decimal(d)) => {
            use rust_decimal::prelude::ToPrimitive;
            d.to_f64().unwrap_or(0.0)
        }
        _ => 0.0,
    }
}

/// Extract string from a literal Expr, defaulting to empty.
fn literal_to_string(expr: &Expr) -> String {
    match expr {
        Expr::Literal(Value::String(s)) => s.clone(),
        _ => String::new(),
    }
}
