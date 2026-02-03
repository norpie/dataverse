//! Core types for transform execution.

use std::collections::HashMap;

use dataverse_lib::model::Record;
use dataverse_lib::model::Value;
use thiserror::Error;
use uuid::Uuid;

// =============================================================================
// Execution Context
// =============================================================================

/// Context available during transform execution.
pub struct TransformContext<'a> {
    /// The source record being transformed.
    pub source_record: &'a Record,
    /// Computed variables (keyed by name without $ prefix).
    pub variables: &'a HashMap<String, Value>,
    /// System variables.
    pub system_vars: SystemVars,
    /// Target cache for find() resolution.
    pub target_cache: &'a dyn TargetCache,
}

/// System variables available in transforms and conditions.
#[derive(Debug, Clone)]
pub struct SystemVars {
    /// Current value in the transform chain (`#value`).
    pub value: Value,
    /// Type annotation of current lookup field (`#type`).
    /// Set by `copy` when extracting from a lookup field.
    pub value_type: Option<String>,
    /// Record index in the current batch (`#index`).
    pub index: usize,
    /// Logical name of source entity (`#source_entity`).
    pub source_entity: String,
    /// Logical name of target entity (`#target_entity`).
    pub target_entity: String,
}

impl SystemVars {
    /// Creates new system variables for starting a transform chain.
    pub fn new(source_entity: String, target_entity: String, index: usize) -> Self {
        Self {
            value: Value::Null,
            value_type: None,
            index,
            source_entity,
            target_entity,
        }
    }

    /// Updates the current value in the chain.
    pub fn with_value(mut self, value: Value) -> Self {
        self.value = value;
        self
    }

    /// Updates the value type annotation.
    pub fn with_value_type(mut self, value_type: Option<String>) -> Self {
        self.value_type = value_type;
        self
    }
}

// =============================================================================
// Transform Results
// =============================================================================

/// Result of executing a single transform.
#[derive(Debug, Clone)]
pub enum TransformResult {
    /// Continue chain with this value.
    Value(Value),
    /// Guard triggered - exit current scope with this value.
    Exit(Value),
    /// Transform failed.
    Error(TransformError),
}

impl TransformResult {
    /// Creates a successful value result.
    pub fn value(v: impl Into<Value>) -> Self {
        Self::Value(v.into())
    }

    /// Creates an exit result (guard triggered).
    pub fn exit(v: impl Into<Value>) -> Self {
        Self::Exit(v.into())
    }

    /// Creates an error result.
    pub fn error(e: TransformError) -> Self {
        Self::Error(e)
    }

    /// Returns true if this is an exit result.
    pub fn is_exit(&self) -> bool {
        matches!(self, Self::Exit(_))
    }

    /// Returns true if this is an error result.
    pub fn is_error(&self) -> bool {
        matches!(self, Self::Error(_))
    }

    /// Extracts the value if this is a Value or Exit result.
    pub fn into_value(self) -> Option<Value> {
        match self {
            Self::Value(v) | Self::Exit(v) => Some(v),
            Self::Error(_) => None,
        }
    }
}

// =============================================================================
// Errors
// =============================================================================

/// Errors that can occur during transform execution.
#[derive(Debug, Clone, Error)]
pub enum TransformError {
    /// Type mismatch in transform input.
    #[error("Type mismatch: expected {expected}, got {got}")]
    TypeMismatch { expected: String, got: String },

    /// Field path not found in source record.
    #[error("Path not found: {path}")]
    PathNotFound { path: String },

    /// Variable not found.
    #[error("Variable not found: ${name}")]
    VariableNotFound { name: String },

    /// Find expression returned no results.
    #[error("Find returned no results for {entity}: {message}")]
    FindNotFound { entity: String, message: String },

    /// Find expression returned multiple results when one expected.
    #[error("Find returned {count} results for {entity}, expected 1")]
    FindMultiple { entity: String, count: usize },

    /// Invalid regex pattern.
    #[error("Invalid regex pattern '{pattern}': {message}")]
    RegexError { pattern: String, message: String },

    /// Lua script error.
    #[error("Lua error: {message}")]
    LuaError { message: String },

    /// Value mapping not found.
    #[error("No mapping found for value: {value:?}")]
    ValueMapNotFound { value: Value },

    /// Division by zero.
    #[error("Division by zero")]
    DivisionByZero,

    /// Parse error.
    #[error("Parse error: {message}")]
    ParseError { message: String },

    /// Invalid date format.
    #[error("Invalid date format '{format}': {message}")]
    DateFormatError { format: String, message: String },

    /// No matching branch in match expression.
    #[error("No matching branch and no default case")]
    NoMatchingBranch,

    /// Coalesce with no non-null values.
    #[error("Coalesce: all values were null")]
    CoalesceAllNull,

    /// Generic transform error.
    #[error("{message}")]
    Other { message: String },
}

impl TransformError {
    /// Creates a type mismatch error.
    pub fn type_mismatch(expected: impl Into<String>, got: impl Into<String>) -> Self {
        Self::TypeMismatch {
            expected: expected.into(),
            got: got.into(),
        }
    }

    /// Creates a path not found error.
    pub fn path_not_found(path: impl Into<String>) -> Self {
        Self::PathNotFound { path: path.into() }
    }

    /// Creates a variable not found error.
    pub fn variable_not_found(name: impl Into<String>) -> Self {
        Self::VariableNotFound { name: name.into() }
    }

    /// Creates a generic error.
    pub fn other(message: impl Into<String>) -> Self {
        Self::Other {
            message: message.into(),
        }
    }
}

// =============================================================================
// Target Cache Trait
// =============================================================================

/// Error from target cache operations.
#[derive(Debug, Clone, Error)]
pub enum FindError {
    /// No matching record found.
    #[error("No matching record found")]
    NotFound,

    /// Multiple matching records found.
    #[error("Multiple records found: {0}")]
    Multiple(usize),

    /// Cache not populated for this entity.
    #[error("Target cache not populated for entity: {0}")]
    NotCached(String),

    /// Lua resolution error.
    #[error("Lua resolution error: {0}")]
    LuaError(String),

    /// Other error.
    #[error("{0}")]
    Other(String),
}

/// Interface for resolving find() expressions against target data.
///
/// This trait abstracts the target data cache, allowing the transform engine
/// to be tested without a real data pipeline.
pub trait TargetCache: Send + Sync {
    /// Find a record by where-clause conditions.
    ///
    /// Returns the matching record, or an error if not found / multiple found.
    fn find_where(&self, entity: &str, conditions: &[(String, Value)])
        -> Result<Record, FindError>;

    /// Find a record using a Lua script.
    ///
    /// The script's `resolve()` function is called with the source record
    /// and target data. Returns the matched record ID.
    fn find_lua(
        &self,
        entity: &str,
        script: &str,
        source_record: &Record,
    ) -> Result<Uuid, FindError>;

    /// Get a record by ID from the cache.
    ///
    /// Used after find() to access fields from the found record.
    fn get(&self, entity: &str, id: Uuid) -> Option<&Record>;
}

// =============================================================================
// Stub Target Cache (for testing)
// =============================================================================

/// A stub target cache that always returns errors.
///
/// Used during development and testing when no real cache is available.
pub struct StubTargetCache;

impl TargetCache for StubTargetCache {
    fn find_where(
        &self,
        entity: &str,
        _conditions: &[(String, Value)],
    ) -> Result<Record, FindError> {
        Err(FindError::NotCached(entity.to_string()))
    }

    fn find_lua(
        &self,
        entity: &str,
        _script: &str,
        _source_record: &Record,
    ) -> Result<Uuid, FindError> {
        Err(FindError::NotCached(entity.to_string()))
    }

    fn get(&self, _entity: &str, _id: Uuid) -> Option<&Record> {
        None
    }
}
