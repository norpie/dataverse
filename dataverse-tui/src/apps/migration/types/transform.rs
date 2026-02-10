//! Transform types and related structures.

use dataverse_lib::model::OptionInfo;
use dataverse_lib::model::Value;
use dataverse_lib::model::metadata::AttributeType;
use serde::Deserialize;
use serde::Serialize;

use super::condition::Condition;
use super::enums::MathOp;
use super::enums::StringOp;

/// A single mapping from source option set value to target option set value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptionSetMapping {
    /// Source option set value.
    pub from: i32,
    /// Target option set value.
    pub to: i32,
}

/// Option set context captured at configuration time.
///
/// Stores everything needed to display and edit value mappings without
/// re-fetching metadata from the API.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptionSetContext {
    /// Option set logical name (e.g., "statuscode").
    pub name: String,
    /// The attribute type kind (Picklist, State, Status, MultiSelectPicklist).
    pub kind: AttributeType,
    /// Available options with value + label, captured when configured.
    pub options: Vec<OptionInfo>,
}

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
    Match { has_default: bool },
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
    /// String operation (chain multiple for sequence).
    StringOps { op: StringOp },
    /// Option set value mapping (source value -> target value).
    /// Source and target contexts are captured at configuration time.
    ValueMap {
        source: OptionSetContext,
        target: OptionSetContext,
        mappings: Vec<OptionSetMapping>,
    },
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
    /// Execute the default chain (transforms stored as ParentType::FindDefault).
    Default,
}
