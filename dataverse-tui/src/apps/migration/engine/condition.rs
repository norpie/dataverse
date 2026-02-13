//! Condition and expression evaluation for guards and match branches.

use dataverse_lib::model::Value;

use crate::apps::migration::types::CompareOp;
use crate::apps::migration::types::Condition;
use crate::apps::migration::types::Expr;
use crate::apps::migration::types::SystemVar;

use super::transforms::resolve::ResolveContext;
use super::transforms::resolve::resolve_path_str;
use super::types::TransformContext;
use super::types::TransformError;

// =============================================================================
// Expression Resolution
// =============================================================================

/// Resolve an expression to a concrete value.
pub fn resolve_expr(expr: &Expr, ctx: &TransformContext<'_>) -> Result<Value, TransformError> {
    match expr {
        Expr::Path(path) => {
            let rctx = ResolveContext {
                source_record: ctx.source_record,
                variables: ctx.variables,
                value: &ctx.system_vars.value,
                value_type: &ctx.system_vars.value_type,
                index: ctx.system_vars.index,
                source_entity: ctx.system_vars.source_entity.clone(),
                target_entity: ctx.system_vars.target_entity.clone(),
                path_cache: ctx.path_cache,
            };
            let (result, _) = resolve_path_str(path, &rctx);
            match result {
                super::types::TransformResult::Value(v) => Ok(v),
                super::types::TransformResult::Error(e) => Err(e),
                super::types::TransformResult::Exit(v) => Ok(v),
            }
        }
        Expr::Variable(name) => ctx
            .variables
            .get(name)
            .cloned()
            .ok_or_else(|| TransformError::variable_not_found(name)),
        Expr::SystemVar(var) => Ok(resolve_system_var(var, ctx)),
        Expr::Literal(value) => Ok(value.clone()),
    }
}

/// Resolve a system variable to a value.
fn resolve_system_var(var: &SystemVar, ctx: &TransformContext<'_>) -> Value {
    match var {
        SystemVar::Value => ctx.system_vars.value.clone(),
        SystemVar::Type => match &ctx.system_vars.value_type {
            Some(entity) => Value::String(entity.name().to_string()),
            None => Value::Null,
        },
        SystemVar::Index => Value::Int(ctx.system_vars.index as i32),
        SystemVar::SourceEntity => Value::String(ctx.system_vars.source_entity.name().to_string()),
        SystemVar::TargetEntity => Value::String(ctx.system_vars.target_entity.name().to_string()),
    }
}

// =============================================================================
// Condition Evaluation
// =============================================================================

/// Evaluate a condition to a boolean.
pub fn evaluate_condition(
    condition: &Condition,
    ctx: &TransformContext<'_>,
) -> Result<bool, TransformError> {
    match condition {
        Condition::And(conditions) => {
            for c in conditions {
                if !evaluate_condition(c, ctx)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        Condition::Or(conditions) => {
            for c in conditions {
                if evaluate_condition(c, ctx)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        Condition::Not(inner) => Ok(!evaluate_condition(inner, ctx)?),
        Condition::Compare { left, op, right } => {
            let left_val = resolve_expr(left, ctx)?;
            let right_val = resolve_expr(right, ctx)?;
            Ok(compare_values(&left_val, op, &right_val))
        }
        Condition::IsNull(expr) => {
            let val = resolve_expr(expr, ctx)?;
            Ok(matches!(val, Value::Null))
        }
        Condition::IsNotNull(expr) => {
            let val = resolve_expr(expr, ctx)?;
            Ok(!matches!(val, Value::Null))
        }
        Condition::Contains { value, substring } => {
            let val = resolve_expr(value, ctx)?;
            let sub = resolve_expr(substring, ctx)?;
            Ok(string_contains(&val, &sub))
        }
        Condition::StartsWith { value, prefix } => {
            let val = resolve_expr(value, ctx)?;
            let pfx = resolve_expr(prefix, ctx)?;
            Ok(string_starts_with(&val, &pfx))
        }
        Condition::EndsWith { value, suffix } => {
            let val = resolve_expr(value, ctx)?;
            let sfx = resolve_expr(suffix, ctx)?;
            let result = string_ends_with(&val, &sfx);
            log::trace!(
                "EndsWith: value={:?}, suffix={:?}, result={}",
                val,
                sfx,
                result
            );
            Ok(result)
        }
    }
}

// =============================================================================
// Value Comparison
// =============================================================================

/// Compare two values using the given operator.
fn compare_values(left: &Value, op: &CompareOp, right: &Value) -> bool {
    match op {
        CompareOp::Equal => values_equal(left, right),
        CompareOp::NotEqual => !values_equal(left, right),
        CompareOp::LessThan => values_order(left, right) == Some(std::cmp::Ordering::Less),
        CompareOp::LessThanOrEqual => {
            matches!(
                values_order(left, right),
                Some(std::cmp::Ordering::Less | std::cmp::Ordering::Equal)
            )
        }
        CompareOp::GreaterThan => values_order(left, right) == Some(std::cmp::Ordering::Greater),
        CompareOp::GreaterThanOrEqual => {
            matches!(
                values_order(left, right),
                Some(std::cmp::Ordering::Greater | std::cmp::Ordering::Equal)
            )
        }
    }
}

use super::util::values_equal;

/// Determine ordering between two values for <, >, <=, >= comparisons.
///
/// Returns `None` if values are not comparable (different incompatible types).
fn values_order(a: &Value, b: &Value) -> Option<std::cmp::Ordering> {
    match (a, b) {
        // Numeric comparisons
        (Value::Int(a), Value::Int(b)) => Some(a.cmp(b)),
        (Value::Long(a), Value::Long(b)) => Some(a.cmp(b)),
        (Value::Int(a), Value::Long(b)) => Some((*a as i64).cmp(b)),
        (Value::Long(a), Value::Int(b)) => Some(a.cmp(&(*b as i64))),
        (Value::Float(a), Value::Float(b)) => a.partial_cmp(b),
        (Value::Decimal(a), Value::Decimal(b)) => Some(a.cmp(b)),

        // OptionSet as numeric
        (Value::OptionSet(a), Value::OptionSet(b)) => Some(a.value.cmp(&b.value)),
        (Value::OptionSet(a), Value::Int(b)) => Some(a.value.cmp(b)),
        (Value::Int(a), Value::OptionSet(b)) => Some(a.cmp(&b.value)),

        // String comparison (lexicographic)
        (Value::String(a), Value::String(b)) => Some(a.cmp(b)),

        // Incomparable types
        _ => None,
    }
}

// =============================================================================
// String Operations
// =============================================================================

/// Extract string value, returning empty string for non-string/null types.
fn as_str(value: &Value) -> &str {
    match value {
        Value::String(s) => s.as_str(),
        _ => "",
    }
}

fn string_contains(value: &Value, substring: &Value) -> bool {
    as_str(value).contains(as_str(substring))
}

fn string_starts_with(value: &Value, prefix: &Value) -> bool {
    as_str(value).starts_with(as_str(prefix))
}

fn string_ends_with(value: &Value, suffix: &Value) -> bool {
    as_str(value).ends_with(as_str(suffix))
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
    use crate::apps::migration::types::CompareOp;
    use crate::apps::migration::types::Condition;
    use crate::apps::migration::types::Expr;
    use crate::apps::migration::types::SystemVar;

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
            system_vars: SystemVars::new(Entity::logical("account"), Entity::logical("contact"), 5),
            find_cache: cache,
            path_cache,
        }
    }

    // =========================================================================
    // resolve_expr tests
    // =========================================================================

    #[test]
    fn resolve_path() {
        let source = Record::new("account").set("name", "Contoso");
        let variables = HashMap::new();
        let cache = StubFindCache;
        let ctx = make_context(&source, &variables, &cache);

        let result = resolve_expr(&Expr::Path("name".to_string()), &ctx).unwrap();
        assert_eq!(result, Value::String("Contoso".to_string()));
    }

    #[test]
    fn resolve_path_missing_errors() {
        let source = Record::new("account");
        let variables = HashMap::new();
        let cache = StubFindCache;
        let ctx = make_context(&source, &variables, &cache);

        let result = resolve_expr(&Expr::Path("nonexistent".to_string()), &ctx);
        assert!(result.is_err());
    }

    #[test]
    fn resolve_variable() {
        let source = Record::new("account");
        let mut variables = HashMap::new();
        variables.insert(
            "owner_email".to_string(),
            Value::String("a@b.com".to_string()),
        );
        let cache = StubFindCache;
        let ctx = make_context(&source, &variables, &cache);

        let result = resolve_expr(&Expr::Variable("owner_email".to_string()), &ctx).unwrap();
        assert_eq!(result, Value::String("a@b.com".to_string()));
    }

    #[test]
    fn resolve_variable_missing_errors() {
        let source = Record::new("account");
        let variables = HashMap::new();
        let cache = StubFindCache;
        let ctx = make_context(&source, &variables, &cache);

        let result = resolve_expr(&Expr::Variable("missing".to_string()), &ctx);
        assert!(matches!(
            result,
            Err(TransformError::VariableNotFound { .. })
        ));
    }

    #[test]
    fn resolve_system_var_value() {
        let source = Record::new("account");
        let variables = HashMap::new();
        let cache = StubFindCache;
        let mut ctx = make_context(&source, &variables, &cache);
        ctx.system_vars.value = Value::Int(42);

        let result = resolve_expr(&Expr::SystemVar(SystemVar::Value), &ctx).unwrap();
        assert_eq!(result, Value::Int(42));
    }

    #[test]
    fn resolve_system_var_index() {
        let source = Record::new("account");
        let variables = HashMap::new();
        let cache = StubFindCache;
        let ctx = make_context(&source, &variables, &cache);

        let result = resolve_expr(&Expr::SystemVar(SystemVar::Index), &ctx).unwrap();
        assert_eq!(result, Value::Int(5));
    }

    #[test]
    fn resolve_system_var_source_entity() {
        let source = Record::new("account");
        let variables = HashMap::new();
        let cache = StubFindCache;
        let ctx = make_context(&source, &variables, &cache);

        let result = resolve_expr(&Expr::SystemVar(SystemVar::SourceEntity), &ctx).unwrap();
        assert_eq!(result, Value::String("account".to_string()));
    }

    #[test]
    fn resolve_system_var_type_none() {
        let source = Record::new("account");
        let variables = HashMap::new();
        let cache = StubFindCache;
        let ctx = make_context(&source, &variables, &cache);

        let result = resolve_expr(&Expr::SystemVar(SystemVar::Type), &ctx).unwrap();
        assert_eq!(result, Value::Null);
    }

    #[test]
    fn resolve_system_var_type_set() {
        let source = Record::new("account");
        let variables = HashMap::new();
        let cache = StubFindCache;
        let mut ctx = make_context(&source, &variables, &cache);
        ctx.system_vars.value_type = Some(Entity::logical("systemuser"));

        let result = resolve_expr(&Expr::SystemVar(SystemVar::Type), &ctx).unwrap();
        assert_eq!(result, Value::String("systemuser".to_string()));
    }

    #[test]
    fn resolve_literal() {
        let source = Record::new("account");
        let variables = HashMap::new();
        let cache = StubFindCache;
        let ctx = make_context(&source, &variables, &cache);

        let result = resolve_expr(&Expr::Literal(Value::Int(99)), &ctx).unwrap();
        assert_eq!(result, Value::Int(99));
    }

    // =========================================================================
    // evaluate_condition tests
    // =========================================================================

    #[test]
    fn compare_equal_ints() {
        let source = Record::new("account").set("status", 1);
        let variables = HashMap::new();
        let cache = StubFindCache;
        let ctx = make_context(&source, &variables, &cache);

        let cond = Condition::Compare {
            left: Expr::Path("status".to_string()),
            op: CompareOp::Equal,
            right: Expr::Literal(Value::Int(1)),
        };
        assert!(evaluate_condition(&cond, &ctx).unwrap());
    }

    #[test]
    fn compare_not_equal() {
        let source = Record::new("account").set("status", 1);
        let variables = HashMap::new();
        let cache = StubFindCache;
        let ctx = make_context(&source, &variables, &cache);

        let cond = Condition::Compare {
            left: Expr::Path("status".to_string()),
            op: CompareOp::Equal,
            right: Expr::Literal(Value::Int(2)),
        };
        assert!(!evaluate_condition(&cond, &ctx).unwrap());
    }

    #[test]
    fn compare_less_than() {
        let source = Record::new("account").set("revenue", 100);
        let variables = HashMap::new();
        let cache = StubFindCache;
        let ctx = make_context(&source, &variables, &cache);

        let cond = Condition::Compare {
            left: Expr::Path("revenue".to_string()),
            op: CompareOp::LessThan,
            right: Expr::Literal(Value::Int(200)),
        };
        assert!(evaluate_condition(&cond, &ctx).unwrap());
    }

    #[test]
    fn compare_strings() {
        let source = Record::new("account").set("name", "Contoso");
        let variables = HashMap::new();
        let cache = StubFindCache;
        let ctx = make_context(&source, &variables, &cache);

        let cond = Condition::Compare {
            left: Expr::Path("name".to_string()),
            op: CompareOp::Equal,
            right: Expr::Literal(Value::String("Contoso".to_string())),
        };
        assert!(evaluate_condition(&cond, &ctx).unwrap());
    }

    #[test]
    fn is_null_true() {
        let source = Record::new("account").set("description", Value::Null);
        let variables = HashMap::new();
        let cache = StubFindCache;
        let ctx = make_context(&source, &variables, &cache);

        let cond = Condition::IsNull(Expr::Path("description".to_string()));
        assert!(evaluate_condition(&cond, &ctx).unwrap());
    }

    #[test]
    fn is_null_false() {
        let source = Record::new("account").set("name", "Contoso");
        let variables = HashMap::new();
        let cache = StubFindCache;
        let ctx = make_context(&source, &variables, &cache);

        let cond = Condition::IsNull(Expr::Path("name".to_string()));
        assert!(!evaluate_condition(&cond, &ctx).unwrap());
    }

    #[test]
    fn is_not_null() {
        let source = Record::new("account").set("name", "Contoso");
        let variables = HashMap::new();
        let cache = StubFindCache;
        let ctx = make_context(&source, &variables, &cache);

        let cond = Condition::IsNotNull(Expr::Path("name".to_string()));
        assert!(evaluate_condition(&cond, &ctx).unwrap());
    }

    #[test]
    fn and_all_true() {
        let source = Record::new("account")
            .set("status", 1)
            .set("name", "Contoso");
        let variables = HashMap::new();
        let cache = StubFindCache;
        let ctx = make_context(&source, &variables, &cache);

        let cond = Condition::And(vec![
            Condition::Compare {
                left: Expr::Path("status".to_string()),
                op: CompareOp::Equal,
                right: Expr::Literal(Value::Int(1)),
            },
            Condition::IsNotNull(Expr::Path("name".to_string())),
        ]);
        assert!(evaluate_condition(&cond, &ctx).unwrap());
    }

    #[test]
    fn and_short_circuits() {
        let source = Record::new("account").set("status", 2);
        let variables = HashMap::new();
        let cache = StubFindCache;
        let ctx = make_context(&source, &variables, &cache);

        let cond = Condition::And(vec![
            Condition::Compare {
                left: Expr::Path("status".to_string()),
                op: CompareOp::Equal,
                right: Expr::Literal(Value::Int(1)),
            },
            // This would error if evaluated (missing field), but shouldn't be reached
            Condition::IsNotNull(Expr::Path("nonexistent".to_string())),
        ]);
        assert!(!evaluate_condition(&cond, &ctx).unwrap());
    }

    #[test]
    fn or_first_true() {
        let source = Record::new("account").set("status", 1);
        let variables = HashMap::new();
        let cache = StubFindCache;
        let ctx = make_context(&source, &variables, &cache);

        let cond = Condition::Or(vec![
            Condition::Compare {
                left: Expr::Path("status".to_string()),
                op: CompareOp::Equal,
                right: Expr::Literal(Value::Int(1)),
            },
            Condition::Compare {
                left: Expr::Path("status".to_string()),
                op: CompareOp::Equal,
                right: Expr::Literal(Value::Int(2)),
            },
        ]);
        assert!(evaluate_condition(&cond, &ctx).unwrap());
    }

    #[test]
    fn or_all_false() {
        let source = Record::new("account").set("status", 3);
        let variables = HashMap::new();
        let cache = StubFindCache;
        let ctx = make_context(&source, &variables, &cache);

        let cond = Condition::Or(vec![
            Condition::Compare {
                left: Expr::Path("status".to_string()),
                op: CompareOp::Equal,
                right: Expr::Literal(Value::Int(1)),
            },
            Condition::Compare {
                left: Expr::Path("status".to_string()),
                op: CompareOp::Equal,
                right: Expr::Literal(Value::Int(2)),
            },
        ]);
        assert!(!evaluate_condition(&cond, &ctx).unwrap());
    }

    #[test]
    fn not_negates() {
        let source = Record::new("account").set("status", 1);
        let variables = HashMap::new();
        let cache = StubFindCache;
        let ctx = make_context(&source, &variables, &cache);

        let cond = Condition::Not(Box::new(Condition::Compare {
            left: Expr::Path("status".to_string()),
            op: CompareOp::Equal,
            right: Expr::Literal(Value::Int(1)),
        }));
        assert!(!evaluate_condition(&cond, &ctx).unwrap());
    }

    #[test]
    fn contains_string() {
        let source = Record::new("account").set("name", "Contoso Ltd");
        let variables = HashMap::new();
        let cache = StubFindCache;
        let ctx = make_context(&source, &variables, &cache);

        let cond = Condition::Contains {
            value: Expr::Path("name".to_string()),
            substring: Expr::Literal(Value::String("toso".to_string())),
        };
        assert!(evaluate_condition(&cond, &ctx).unwrap());
    }

    #[test]
    fn starts_with_string() {
        let source = Record::new("account").set("name", "Contoso Ltd");
        let variables = HashMap::new();
        let cache = StubFindCache;
        let ctx = make_context(&source, &variables, &cache);

        let cond = Condition::StartsWith {
            value: Expr::Path("name".to_string()),
            prefix: Expr::Literal(Value::String("Con".to_string())),
        };
        assert!(evaluate_condition(&cond, &ctx).unwrap());
    }

    #[test]
    fn ends_with_string() {
        let source = Record::new("account").set("name", "Contoso Ltd");
        let variables = HashMap::new();
        let cache = StubFindCache;
        let ctx = make_context(&source, &variables, &cache);

        let cond = Condition::EndsWith {
            value: Expr::Path("name".to_string()),
            suffix: Expr::Literal(Value::String("Ltd".to_string())),
        };
        assert!(evaluate_condition(&cond, &ctx).unwrap());
    }

    #[test]
    fn contains_non_string_returns_false() {
        let source = Record::new("account").set("status", 1);
        let variables = HashMap::new();
        let cache = StubFindCache;
        let ctx = make_context(&source, &variables, &cache);

        let cond = Condition::Contains {
            value: Expr::Path("status".to_string()),
            substring: Expr::Literal(Value::String("1".to_string())),
        };
        assert!(!evaluate_condition(&cond, &ctx).unwrap());
    }

    #[test]
    fn compare_cross_type_int_long() {
        let source = Record::new("account").set("count", Value::Long(42));
        let variables = HashMap::new();
        let cache = StubFindCache;
        let ctx = make_context(&source, &variables, &cache);

        let cond = Condition::Compare {
            left: Expr::Path("count".to_string()),
            op: CompareOp::Equal,
            right: Expr::Literal(Value::Int(42)),
        };
        assert!(evaluate_condition(&cond, &ctx).unwrap());
    }

    #[test]
    fn compare_incomparable_types_not_equal() {
        let source = Record::new("account").set("name", "test");
        let variables = HashMap::new();
        let cache = StubFindCache;
        let ctx = make_context(&source, &variables, &cache);

        // String vs Int — should not be equal
        let cond = Condition::Compare {
            left: Expr::Path("name".to_string()),
            op: CompareOp::Equal,
            right: Expr::Literal(Value::Int(1)),
        };
        assert!(!evaluate_condition(&cond, &ctx).unwrap());
    }

    #[test]
    fn compare_incomparable_types_less_than_false() {
        let source = Record::new("account").set("name", "test");
        let variables = HashMap::new();
        let cache = StubFindCache;
        let ctx = make_context(&source, &variables, &cache);

        // String vs Int ordering — incomparable, returns false
        let cond = Condition::Compare {
            left: Expr::Path("name".to_string()),
            op: CompareOp::LessThan,
            right: Expr::Literal(Value::Int(1)),
        };
        assert!(!evaluate_condition(&cond, &ctx).unwrap());
    }

    #[test]
    fn variable_in_condition() {
        let source = Record::new("account").set("status", 1);
        let mut variables = HashMap::new();
        variables.insert("expected_status".to_string(), Value::Int(1));
        let cache = StubFindCache;
        let ctx = make_context(&source, &variables, &cache);

        let cond = Condition::Compare {
            left: Expr::Path("status".to_string()),
            op: CompareOp::Equal,
            right: Expr::Variable("expected_status".to_string()),
        };
        assert!(evaluate_condition(&cond, &ctx).unwrap());
    }
}
