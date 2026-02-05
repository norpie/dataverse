//! Enumeration types for migration configuration.

use serde::Deserialize;
use serde::Serialize;

/// Execution mode for a phase or entity mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    /// Declarative field mappings and transforms.
    Declarative,
    /// Lua script.
    Lua,
}

/// Strategy for matching source records to target records.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MatchStrategy {
    /// Source and target use identical GUIDs.
    SameId,
    /// Use a find expression to locate target records.
    Find,
}

/// Fallback behavior when no target match is found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NoMatchFallback {
    /// Stop processing, something is wrong.
    Error,
    /// Treat as a new record.
    Create,
    /// Skip this source record.
    Ignore,
}

/// Strategy for handling orphaned target records.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrphanStrategy {
    /// Remove orphaned records.
    Delete,
    /// Deactivate orphaned records.
    Deactivate,
    /// Leave orphaned records untouched.
    Ignore,
    /// Flag orphaned records as errors.
    Error,
}

/// Status of a phase execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PhaseRunStatus {
    /// Currently executing.
    Running,
    /// Completed successfully.
    Completed,
    /// Execution failed.
    Failed,
    /// User cancelled execution.
    Cancelled,
}

/// Type of parent for a transform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ParentType {
    /// Transform belongs to a field mapping.
    FieldMapping,
    /// Transform belongs to a variable.
    Variable,
    /// Transform belongs to a match branch.
    MatchBranch,
    /// Transform belongs to a guard fallback.
    GuardFallback,
    /// Transform belongs to a coalesce chain (one of multiple fallback chains).
    CoalesceChain,
    /// Transform belongs to a find condition (where-clause mode).
    FindCondition,
}

/// String operation type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StringOp {
    Uppercase,
    Lowercase,
    Trim,
    TrimStart,
    TrimEnd,
}

/// Mathematical operation type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MathOp {
    Add(f64),
    Subtract(f64),
    Multiply(f64),
    Divide(f64),
    Round(u32), // decimal places
}

/// Comparison operator for conditions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompareOp {
    Equal,
    NotEqual,
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,
}

/// System variable identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SystemVar {
    /// Current value in the transform chain.
    Value,
    /// Type annotation of current lookup field.
    Type,
    /// Record index in the current batch.
    Index,
    /// Logical name of source entity.
    SourceEntity,
    /// Logical name of target entity.
    TargetEntity,
}

// =============================================================================
// String conversions for database storage
// =============================================================================

impl Mode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Mode::Declarative => "declarative",
            Mode::Lua => "lua",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "declarative" => Some(Mode::Declarative),
            "lua" => Some(Mode::Lua),
            _ => None,
        }
    }
}

impl MatchStrategy {
    pub fn as_str(&self) -> &'static str {
        match self {
            MatchStrategy::SameId => "same_id",
            MatchStrategy::Find => "find",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "same_id" => Some(MatchStrategy::SameId),
            "find" => Some(MatchStrategy::Find),
            _ => None,
        }
    }
}

impl NoMatchFallback {
    pub fn as_str(&self) -> &'static str {
        match self {
            NoMatchFallback::Error => "error",
            NoMatchFallback::Create => "create",
            NoMatchFallback::Ignore => "ignore",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "error" => Some(NoMatchFallback::Error),
            "create" => Some(NoMatchFallback::Create),
            "ignore" => Some(NoMatchFallback::Ignore),
            _ => None,
        }
    }
}

impl OrphanStrategy {
    pub fn as_str(&self) -> &'static str {
        match self {
            OrphanStrategy::Delete => "delete",
            OrphanStrategy::Deactivate => "deactivate",
            OrphanStrategy::Ignore => "ignore",
            OrphanStrategy::Error => "error",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "delete" => Some(OrphanStrategy::Delete),
            "deactivate" => Some(OrphanStrategy::Deactivate),
            "ignore" => Some(OrphanStrategy::Ignore),
            "error" => Some(OrphanStrategy::Error),
            _ => None,
        }
    }
}

impl PhaseRunStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            PhaseRunStatus::Running => "running",
            PhaseRunStatus::Completed => "completed",
            PhaseRunStatus::Failed => "failed",
            PhaseRunStatus::Cancelled => "cancelled",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "running" => Some(PhaseRunStatus::Running),
            "completed" => Some(PhaseRunStatus::Completed),
            "failed" => Some(PhaseRunStatus::Failed),
            "cancelled" => Some(PhaseRunStatus::Cancelled),
            _ => None,
        }
    }
}

impl ParentType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ParentType::FieldMapping => "field_mapping",
            ParentType::Variable => "variable",
            ParentType::MatchBranch => "match_branch",
            ParentType::GuardFallback => "guard_fallback",
            ParentType::CoalesceChain => "coalesce_chain",
            ParentType::FindCondition => "find_condition",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "field_mapping" => Some(ParentType::FieldMapping),
            "variable" => Some(ParentType::Variable),
            "match_branch" => Some(ParentType::MatchBranch),
            "guard_fallback" => Some(ParentType::GuardFallback),
            "coalesce_chain" => Some(ParentType::CoalesceChain),
            "find_condition" => Some(ParentType::FindCondition),
            _ => None,
        }
    }
}
