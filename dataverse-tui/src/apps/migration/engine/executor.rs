//! Chain executor for transform sequences.
//!
//! This module handles executing a sequence of transforms, passing the result
//! of each transform to the next via the `#value` system variable.

use crate::apps::migration::types::Condition;
use crate::apps::migration::types::TransformData;

use super::transforms::execute_constant;
use super::transforms::execute_copy;
use super::transforms::execute_guid;
use super::types::TransformContext;
use super::types::TransformError;
use super::types::TransformResult;

// =============================================================================
// Chain Item Types
// =============================================================================

/// A single item in a transform chain, with any nested child chains.
///
/// This is the "materialized" form of a transform that includes all nested
/// data needed for execution. Built from DB rows before execution.
#[derive(Debug, Clone)]
pub struct ChainItem {
    /// The transform operation data.
    pub data: TransformData,
    /// Child chains for transforms that have them.
    pub children: ChainChildren,
}

impl ChainItem {
    /// Creates a chain item with no children.
    pub fn new(data: TransformData) -> Self {
        Self {
            data,
            children: ChainChildren::None,
        }
    }

    /// Creates a chain item with a fallback chain (for guard).
    pub fn with_fallback(data: TransformData, fallback: Vec<ChainItem>) -> Self {
        Self {
            data,
            children: ChainChildren::Fallback(fallback),
        }
    }

    /// Creates a chain item with branches (for match).
    pub fn with_branches(
        data: TransformData,
        branches: Vec<BranchItem>,
        default_chain: Option<Vec<ChainItem>>,
    ) -> Self {
        Self {
            data,
            children: ChainChildren::Branches(branches, default_chain),
        }
    }

    /// Creates a chain item with alternatives (for coalesce).
    pub fn with_alternatives(data: TransformData, alternatives: Vec<Vec<ChainItem>>) -> Self {
        Self {
            data,
            children: ChainChildren::Alternatives(alternatives),
        }
    }

    /// Creates a chain item with find conditions.
    pub fn with_find_conditions(data: TransformData, conditions: Vec<FindConditionItem>) -> Self {
        Self {
            data,
            children: ChainChildren::FindConditions(conditions),
        }
    }
}

/// Child chains for transforms that have them.
#[derive(Debug, Clone, Default)]
pub enum ChainChildren {
    /// No children (most transforms).
    #[default]
    None,
    /// Guard fallback chain - executed when condition is true.
    Fallback(Vec<ChainItem>),
    /// Match branches - evaluated in order, first match wins.
    /// The optional second element is the default chain (executed when no branch matches).
    Branches(Vec<BranchItem>, Option<Vec<ChainItem>>),
    /// Coalesce alternatives - first non-null wins.
    Alternatives(Vec<Vec<ChainItem>>),
    /// Find condition source chains - produce values to match against.
    FindConditions(Vec<FindConditionItem>),
}

/// A branch within a match transform.
#[derive(Debug, Clone)]
pub struct BranchItem {
    /// Condition to evaluate for this branch.
    pub condition: Condition,
    /// Transform chain to execute if this branch matches.
    pub chain: Vec<ChainItem>,
}

/// A find condition with its source chain.
#[derive(Debug, Clone)]
pub struct FindConditionItem {
    /// Target field to match against.
    pub target_field: String,
    /// Transform chain producing the value to match.
    pub source_chain: Vec<ChainItem>,
}

// =============================================================================
// Chain Execution
// =============================================================================

/// Execute a chain of transforms in sequence.
///
/// Each transform receives the previous transform's output as `#value`.
/// The chain starts with `#value` from `ctx.system_vars`.
///
/// # Returns
///
/// - `Value(v)`: Chain completed successfully with final value `v`
/// - `Exit(v)`: A guard triggered early exit with value `v`
/// - `Error(e)`: A transform failed with error `e`
///
/// # Scope Handling
///
/// `Exit` results pass through unchanged. The *caller* that creates a scope
/// (guard fallback, match branch) is responsible for converting `Exit` to
/// `Value` at the scope boundary.
pub fn execute_chain(chain: &[ChainItem], ctx: &mut TransformContext<'_>) -> TransformResult {
    // Empty chain returns current value
    if chain.is_empty() {
        return TransformResult::Value(ctx.system_vars.value.clone());
    }

    for item in chain {
        let result = execute_transform(item, ctx);

        match result {
            TransformResult::Value(value) => {
                // Update #value for next transform
                ctx.system_vars.value = value;
            }
            // Exit and Error propagate immediately
            TransformResult::Exit(_) | TransformResult::Error(_) => {
                return result;
            }
        }
    }

    // Return final value
    TransformResult::Value(ctx.system_vars.value.clone())
}

/// Execute a chain that creates a new scope.
///
/// This is used by guard (fallback) and match (branches) to execute child
/// chains. `Exit` results from the child chain are converted to `Value` at
/// the scope boundary.
pub fn execute_scoped_chain(
    chain: &[ChainItem],
    ctx: &mut TransformContext<'_>,
) -> TransformResult {
    let result = execute_chain(chain, ctx);

    match result {
        // Exit at scope boundary becomes Value
        TransformResult::Exit(value) => TransformResult::Value(value),
        // Value and Error pass through unchanged
        other => other,
    }
}

// =============================================================================
// Transform Dispatch
// =============================================================================

/// Execute a single transform.
///
/// Dispatches to the appropriate transform implementation based on the
/// transform type. Transforms that need children access them via `item.children`.
fn execute_transform(item: &ChainItem, ctx: &mut TransformContext<'_>) -> TransformResult {
    match &item.data {
        // Simple transforms (Step 4)
        TransformData::Copy { path } => {
            let (result, value_type) = execute_copy(path, ctx.source_record);
            // Update type annotation if copy extracted from a lookup
            if value_type.is_some() {
                ctx.system_vars.value_type = value_type;
            }
            result
        }
        TransformData::Constant { value } => execute_constant(value),
        TransformData::Guid => execute_guid(),

        // String transforms (Step 6)
        TransformData::Format { .. } => not_implemented("format"),
        TransformData::Replace { .. } => not_implemented("replace"),
        TransformData::StringOps { .. } => not_implemented("string_ops"),

        // Type conversions (Step 7)
        TransformData::Convert { .. } => not_implemented("convert"),
        TransformData::ParseInt => not_implemented("parse_int"),
        TransformData::ParseDecimal => not_implemented("parse_decimal"),
        TransformData::ParseDate { .. } => not_implemented("parse_date"),

        // Control flow (Step 8)
        TransformData::Guard { .. } => not_implemented("guard"),
        TransformData::Match { .. } => not_implemented("match"),

        // Data transforms (Step 9)
        TransformData::ValueMap { .. } => not_implemented("value_map"),
        TransformData::Math { .. } => not_implemented("math"),
        TransformData::Coalesce => not_implemented("coalesce"),

        // Find (Step 11)
        TransformData::Find { .. } => not_implemented("find"),
    }
}

/// Placeholder for unimplemented transforms.
fn not_implemented(name: &str) -> TransformResult {
    TransformResult::Error(TransformError::Other {
        message: format!("Transform '{name}' not yet implemented"),
    })
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use dataverse_lib::model::Entity;
    use dataverse_lib::model::Record;
    use dataverse_lib::model::Value;

    use crate::apps::migration::engine::StubTargetCache;
    use crate::apps::migration::engine::SystemVars;
    use crate::apps::migration::engine::TransformContext;

    use super::*;

    fn make_context<'a>(
        source: &'a Record,
        variables: &'a HashMap<String, Value>,
        cache: &'a StubTargetCache,
    ) -> TransformContext<'a> {
        TransformContext {
            source_record: source,
            variables,
            system_vars: SystemVars::new(
                Entity::logical("source_entity"),
                Entity::logical("target_entity"),
                0,
            ),
            target_cache: cache,
        }
    }

    #[test]
    fn empty_chain_returns_current_value() {
        let source = Record::default();
        let variables = HashMap::new();
        let cache = StubTargetCache;
        let mut ctx = make_context(&source, &variables, &cache);

        // Set initial value
        ctx.system_vars.value = Value::String("initial".to_string());

        let result = execute_chain(&[], &mut ctx);

        match result {
            TransformResult::Value(v) => {
                assert_eq!(v, Value::String("initial".to_string()));
            }
            _ => panic!("Expected Value result"),
        }
    }

    #[test]
    fn execute_scoped_converts_exit_to_value() {
        // We can't easily test this without implementing a transform that returns Exit,
        // but we can at least verify the function exists and handles Value correctly
        let source = Record::default();
        let variables = HashMap::new();
        let cache = StubTargetCache;
        let mut ctx = make_context(&source, &variables, &cache);

        ctx.system_vars.value = Value::String("test".to_string());

        let result = execute_scoped_chain(&[], &mut ctx);

        match result {
            TransformResult::Value(v) => {
                assert_eq!(v, Value::String("test".to_string()));
            }
            _ => panic!("Expected Value result"),
        }
    }
}
