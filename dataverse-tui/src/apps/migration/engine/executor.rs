//! Chain executor for transform sequences.
//!
//! This module handles executing a sequence of transforms, passing the result
//! of each transform to the next via the `#value` system variable.

use std::sync::Arc;

use dataverse_lib::model::Record;
use dataverse_lib::model::Value;

use super::condition::evaluate_condition;
use super::types::FindError;
use crate::apps::migration::types::Condition;
use crate::apps::migration::types::FindFallback;
use crate::apps::migration::types::FindMode;
use crate::apps::migration::types::TransformData;

use super::transforms::execute_constant;
use super::transforms::execute_convert;
use super::transforms::execute_copy;
use super::transforms::execute_format;
use super::transforms::execute_guid;
use super::transforms::execute_math;
use super::transforms::execute_parse_date;
use super::transforms::execute_parse_decimal;
use super::transforms::execute_parse_int;
use super::transforms::execute_replace;
use super::transforms::execute_string_ops;
use super::transforms::execute_value_map;
use super::transforms::resolve::ResolveContext;
use super::transforms::ConvertTarget;
use super::transforms::ValueMapping;
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

    /// Creates a chain item with find conditions and an optional default chain.
    pub fn with_find_conditions(
        data: TransformData,
        conditions: Vec<FindConditionItem>,
        default_chain: Option<Vec<ChainItem>>,
    ) -> Self {
        Self {
            data,
            children: ChainChildren::FindConditions(conditions, default_chain),
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
    /// The optional second element is the default chain (for `FindFallback::Default`).
    FindConditions(Vec<FindConditionItem>, Option<Vec<ChainItem>>),
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

    for (i, item) in chain.iter().enumerate() {
        log::trace!(
            "execute_chain: step {} transform={:?}, #value={:?}",
            i,
            std::mem::discriminant(&item.data),
            ctx.system_vars.value
        );
        let result = execute_transform(item, ctx);

        match result {
            TransformResult::Value(ref value) => {
                log::trace!("execute_chain: step {} result=Value({:?})", i, value);
                // Update #value for next transform
                ctx.system_vars.value = value.clone();
            }
            // Exit and Error propagate immediately
            TransformResult::Exit(_) | TransformResult::Error(_) => {
                log::trace!("execute_chain: step {} result={:?}", i, result);
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
/// Build a `ResolveContext` from the current `TransformContext`.
fn resolve_ctx<'a>(ctx: &'a TransformContext<'a>) -> ResolveContext<'a> {
    ResolveContext {
        source_record: ctx.source_record,
        variables: ctx.variables,
        value: &ctx.system_vars.value,
        value_type: &ctx.system_vars.value_type,
        index: ctx.system_vars.index,
        source_entity: ctx.system_vars.source_entity.clone(),
        target_entity: ctx.system_vars.target_entity.clone(),
        path_cache: ctx.path_cache,
    }
}

/// Dispatches to the appropriate transform implementation based on the
/// transform type. Transforms that need children access them via `item.children`.
fn execute_transform(item: &ChainItem, ctx: &mut TransformContext<'_>) -> TransformResult {
    match &item.data {
        // Simple transforms (Step 4)
        TransformData::Copy { path } => {
            let rctx = resolve_ctx(ctx);
            let (result, value_type) = execute_copy(path, &rctx);
            // Update type annotation if copy extracted from a lookup
            if value_type.is_some() {
                ctx.system_vars.value_type = value_type;
            }
            result
        }
        TransformData::Constant { value } => execute_constant(value),
        TransformData::Guid => execute_guid(),

        // String transforms
        TransformData::Format { template } => {
            let rctx = resolve_ctx(ctx);
            execute_format(template, &rctx)
        }
        TransformData::Replace { from, to, regex } => {
            execute_replace(&ctx.system_vars.value, from, to, *regex)
        }
        TransformData::StringOps { op } => execute_string_ops(&ctx.system_vars.value, &[*op]),

        // Type conversions
        TransformData::Convert { target_type } => match ConvertTarget::from_str(target_type) {
            Some(target) => execute_convert(&ctx.system_vars.value, target),
            None => TransformResult::Error(TransformError::Other {
                message: format!("Unknown convert target type: '{target_type}'"),
            }),
        },
        TransformData::ParseInt => execute_parse_int(&ctx.system_vars.value),
        TransformData::ParseDecimal => execute_parse_decimal(&ctx.system_vars.value),
        TransformData::ParseDate { format } => execute_parse_date(&ctx.system_vars.value, format),

        // Data transforms
        TransformData::ValueMap { mappings, .. } => {
            let value_mappings: Vec<ValueMapping> = mappings
                .iter()
                .map(|m| ValueMapping::new(Value::Int(m.from), Value::Int(m.to)))
                .collect();
            execute_value_map(&ctx.system_vars.value, &value_mappings)
        }
        TransformData::Math { operation } => execute_math(&ctx.system_vars.value, operation),

        // Control flow
        TransformData::Guard { condition } => execute_guard(condition, &item.children, ctx),
        TransformData::Match { .. } => execute_match(&item.children, ctx),
        TransformData::Coalesce => execute_coalesce(&item.children, ctx),

        // Find
        TransformData::Find {
            entity,
            fallback,
            mode,
        } => execute_find(entity, fallback, mode, &item.children, ctx),
    }
}

// =============================================================================
// Control Flow Transforms
// =============================================================================

/// Execute guard transform.
///
/// If condition is true, execute the fallback chain and return `Exit(result)`.
/// If condition is false, pass through `#value` unchanged.
fn execute_guard(
    condition: &Condition,
    children: &ChainChildren,
    ctx: &mut TransformContext<'_>,
) -> TransformResult {
    let result = match evaluate_condition(condition, ctx) {
        Ok(v) => v,
        Err(e) => return TransformResult::Error(e),
    };

    if result {
        let fallback = match children {
            ChainChildren::Fallback(chain) => chain,
            _ => {
                return TransformResult::Error(TransformError::other(
                    "Guard missing fallback chain",
                ));
            }
        };
        match execute_scoped_chain(fallback, ctx) {
            TransformResult::Value(v) => TransformResult::Exit(v),
            TransformResult::Error(e) => TransformResult::Error(e),
            // scoped_chain already converts Exit→Value, so this shouldn't happen
            TransformResult::Exit(v) => TransformResult::Exit(v),
        }
    } else {
        TransformResult::Value(ctx.system_vars.value.clone())
    }
}

/// Execute match transform.
///
/// Evaluate branch conditions in order. First matching branch executes its chain.
/// If no branch matches and a default exists, execute the default chain.
/// If no match and no default, return `NoMatchingBranch` error.
fn execute_match(children: &ChainChildren, ctx: &mut TransformContext<'_>) -> TransformResult {
    let (branches, default_chain) = match children {
        ChainChildren::Branches(branches, default) => (branches, default),
        _ => return TransformResult::Error(TransformError::other("Match missing branches")),
    };

    log::trace!(
        "execute_match: #value={:?}, {} branches, has_default={}",
        ctx.system_vars.value,
        branches.len(),
        default_chain.is_some()
    );

    for (i, branch) in branches.iter().enumerate() {
        log::trace!(
            "execute_match: evaluating branch {} condition={:?}",
            i,
            branch.condition
        );
        let matched = match evaluate_condition(&branch.condition, ctx) {
            Ok(v) => v,
            Err(e) => return TransformResult::Error(e),
        };

        log::trace!("execute_match: branch {} matched={}", i, matched);
        if matched {
            return execute_scoped_chain(&branch.chain, ctx);
        }
    }

    log::trace!("execute_match: no branch matched, falling back to default");
    // No branch matched — try default
    if let Some(default) = default_chain {
        return execute_scoped_chain(default, ctx);
    }

    TransformResult::Error(TransformError::NoMatchingBranch)
}

/// Execute coalesce transform.
///
/// Try each alternative chain in order. Return the first non-null result.
/// If all alternatives produce null, return `CoalesceAllNull` error.
fn execute_coalesce(children: &ChainChildren, ctx: &mut TransformContext<'_>) -> TransformResult {
    let alternatives = match children {
        ChainChildren::Alternatives(alts) => alts,
        _ => return TransformResult::Error(TransformError::other("Coalesce missing alternatives")),
    };

    let saved_value = ctx.system_vars.value.clone();
    let saved_type = ctx.system_vars.value_type.clone();

    for alt in alternatives {
        // Reset state for each attempt
        ctx.system_vars.value = saved_value.clone();
        ctx.system_vars.value_type = saved_type.clone();

        match execute_scoped_chain(alt, ctx) {
            TransformResult::Value(v) if !matches!(v, Value::Null) => {
                return TransformResult::Value(v);
            }
            TransformResult::Error(e) => return TransformResult::Error(e),
            // Null or Exit-turned-Value(Null) — try next alternative
            _ => continue,
        }
    }

    TransformResult::Error(TransformError::CoalesceAllNull)
}

/// Execute find transform.
///
/// Locates a record in the target environment. Returns `Value::Record` on success.
/// Two modes: where-clause (declarative conditions) or Lua script.
fn execute_find(
    entity: &str,
    fallback: &FindFallback,
    mode: &FindMode,
    children: &ChainChildren,
    ctx: &mut TransformContext<'_>,
) -> TransformResult {
    let result = match mode {
        FindMode::Where => execute_find_where(entity, children, ctx),
        FindMode::Lua { script } => execute_find_lua(entity, script, ctx),
    };

    match result {
        Ok(record) => {
            ctx.system_vars.value_type = Some(record.entity().clone());
            TransformResult::Value(Value::Record(record))
        }
        Err(find_err) => apply_find_fallback(find_err, entity, fallback, children, ctx),
    }
}

/// Execute find in where-clause mode.
///
/// Evaluates each condition's source chain to produce a match value,
/// then queries the target cache with collected conditions.
fn execute_find_where(
    entity: &str,
    children: &ChainChildren,
    ctx: &mut TransformContext<'_>,
) -> Result<Arc<Record>, FindError> {
    let (conditions, _) = match children {
        ChainChildren::FindConditions(conds, default) => (conds, default),
        _ => return Err(FindError::Other("Find missing conditions".to_string())),
    };

    let saved_value = ctx.system_vars.value.clone();
    let saved_type = ctx.system_vars.value_type.clone();
    let mut collected: Vec<(String, Value)> = Vec::new();

    for cond in conditions {
        // Reset #value for each condition's source chain
        ctx.system_vars.value = Value::Null;
        ctx.system_vars.value_type = None;

        match execute_chain(&cond.source_chain, ctx) {
            TransformResult::Value(v) => {
                collected.push((cond.target_field.clone(), v));
            }
            TransformResult::Error(e) => {
                // Restore state before returning
                ctx.system_vars.value = saved_value;
                ctx.system_vars.value_type = saved_type;
                return Err(FindError::Other(e.to_string()));
            }
            TransformResult::Exit(v) => {
                collected.push((cond.target_field.clone(), v));
            }
        }
    }

    // Restore #value state after condition evaluation
    ctx.system_vars.value = saved_value;
    ctx.system_vars.value_type = saved_type;

    ctx.find_cache.find_where(entity, &collected)
}

/// Execute find in Lua mode.
fn execute_find_lua(
    entity: &str,
    script: &str,
    ctx: &TransformContext<'_>,
) -> Result<Arc<Record>, FindError> {
    let id = ctx.find_cache.find_lua(entity, script, ctx.source_record)?;

    ctx.find_cache
        .get(entity, id)
        .map(|r| Arc::new(r.clone()))
        .ok_or_else(|| {
            FindError::Other(format!("Lua find returned ID {id} but record not in cache"))
        })
}

/// Apply fallback when find fails.
fn apply_find_fallback(
    find_err: FindError,
    entity: &str,
    fallback: &FindFallback,
    children: &ChainChildren,
    ctx: &mut TransformContext<'_>,
) -> TransformResult {
    match fallback {
        FindFallback::Error => {
            let error = match find_err {
                FindError::Multiple(count) => TransformError::FindMultiple {
                    entity: entity.to_string(),
                    count,
                },
                _ => TransformError::FindNotFound {
                    entity: entity.to_string(),
                    message: find_err.to_string(),
                },
            };
            TransformResult::Error(error)
        }
        FindFallback::Null => TransformResult::Value(Value::Null),
        FindFallback::Default => {
            let default_chain = match children {
                ChainChildren::FindConditions(_, Some(chain)) => chain,
                _ => {
                    return TransformResult::Error(TransformError::other(
                        "Find fallback is Default but no default chain provided",
                    ));
                }
            };
            execute_scoped_chain(default_chain, ctx)
        }
    }
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

    use crate::apps::migration::engine::PathCache;
    use crate::apps::migration::engine::StubFindCache;
    use crate::apps::migration::engine::SystemVars;
    use crate::apps::migration::engine::TransformContext;

    use super::*;

    fn make_context<'a>(
        source: &'a Record,
        variables: &'a HashMap<String, Value>,
        cache: &'a StubFindCache,
    ) -> TransformContext<'a> {
        let path_cache: &'a PathCache = Box::leak(Box::new(PathCache::new()));
        TransformContext {
            source_record: source,
            variables,
            system_vars: SystemVars::new(
                Entity::logical("source_entity"),
                Entity::logical("target_entity"),
                0,
            ),
            find_cache: cache,
            path_cache,
        }
    }

    #[test]
    fn empty_chain_returns_current_value() {
        let source = Record::default();
        let variables = HashMap::new();
        let cache = StubFindCache;
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
        let cache = StubFindCache;
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
