//! Transform types and related structures.

use dataverse_lib::model::Value;
use serde::Deserialize;
use serde::Serialize;

use super::condition::Condition;
use super::enums::MathOp;
use super::enums::StringOp;

/// Transform operation data (excludes nested transforms which are separate DB rows).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TransformData {
    /// Copy value from source field path.
    Copy { path: String },
    /// Static constant value.
    Constant { value: Value },
    /// Guard with condition - early exit if true.
    Guard { condition: Condition },
    /// Match expression - branches are separate rows.
    Match,
    /// Find record in target environment.
    Find {
        entity: String,
        fallback: FindFallback,
        mode: FindMode,
    },
    /// String formatting with template.
    Format { template: String },
    /// String replacement.
    Replace {
        from: String,
        to: String,
        regex: bool,
    },
    /// String operations in sequence.
    StringOps { ops: Vec<StringOp> },
    /// Value mapping lookup table.
    ValueMap { mappings: Vec<(Value, Value)> },
    /// Mathematical operation.
    Math { operation: MathOp },
    /// First non-null value (uses chain).
    Coalesce,
    /// Type conversion.
    Convert { target_type: String },
    /// Parse string to integer.
    ParseInt,
    /// Parse string to decimal.
    ParseDecimal,
    /// Parse string to date.
    ParseDate { format: String },
    /// Generate new GUID.
    Guid,
}

/// Mode for find expression.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FindMode {
    /// Declarative where-clause.
    /// Conditions are stored in the find_conditions table, not inline.
    Where,
    /// Lua script for complex matching.
    Lua { script: String },
}

/// Fallback behavior when find() doesn't match.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FindFallback {
    /// Fail the record transformation.
    Error,
    /// Use null value.
    Null,
    /// Use specified default value.
    Default { value: Value },
}
