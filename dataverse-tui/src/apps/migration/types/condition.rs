//! Condition expression types for guards and match branches.

use dataverse_lib::model::Value;
use serde::Deserialize;
use serde::Serialize;

use super::enums::CompareOp;
use super::enums::SystemVar;

/// A condition expression used in guards and match branches.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Condition {
    /// Logical AND of multiple conditions.
    And(Vec<Condition>),
    /// Logical OR of multiple conditions.
    Or(Vec<Condition>),
    /// Logical NOT.
    Not(Box<Condition>),
    /// Comparison between two expressions.
    Compare {
        left: Expr,
        op: CompareOp,
        right: Expr,
    },
    /// Check if expression is null.
    IsNull(Expr),
    /// Check if expression is not null.
    IsNotNull(Expr),
    /// Check if string contains substring.
    Contains { value: Expr, substring: Expr },
    /// Check if string starts with prefix.
    StartsWith { value: Expr, prefix: Expr },
    /// Check if string ends with suffix.
    EndsWith { value: Expr, suffix: Expr },
}

/// An expression that evaluates to a value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Expr {
    /// Source field path (e.g., "name", "accountid.name").
    Path(String),
    /// User-defined variable (e.g., "$owner").
    Variable(String),
    /// System variable (e.g., #value, #type).
    SystemVar(SystemVar),
    /// Literal constant value.
    Literal(Value),
}
