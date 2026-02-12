//! Variable computation for transform chains.
//!
//! Variables are computed in order before field mappings. Each variable's chain
//! is executed, and the result is stored so later variables (and field mappings)
//! can reference it via `$name`.

use std::collections::HashMap;

use dataverse_lib::model::Value;

use super::executor::execute_chain;
use super::executor::ChainItem;
use super::types::FindCache;
use super::types::PathCache;
use super::types::SystemVars;
use super::types::TransformContext;
use super::types::TransformError;
use super::types::TransformResult;

use dataverse_lib::model::Record;

/// Compute all variables in order, returning the resulting variable map.
///
/// Each variable's chain starts with `#value = Null`. The result is stored
/// in the map under the variable's name. Later variables can reference earlier
/// ones via `$name` in expressions.
///
/// # Errors
///
/// Returns the first variable computation error, along with the variable name.
pub fn compute_variables(
    variables: &[(String, Vec<ChainItem>)],
    source_record: &Record,
    system_vars: &SystemVars,
    find_cache: &dyn FindCache,
    path_cache: &PathCache,
) -> Result<HashMap<String, Value>, (String, TransformError)> {
    let mut computed: HashMap<String, Value> = HashMap::new();

    for (name, chain) in variables {
        let mut ctx = TransformContext {
            source_record,
            variables: &computed,
            system_vars: SystemVars {
                value: Value::Null,
                value_type: None,
                ..system_vars.clone()
            },
            find_cache,
            path_cache,
        };

        match execute_chain(chain, &mut ctx) {
            TransformResult::Value(v) => {
                computed.insert(name.clone(), v);
            }
            TransformResult::Exit(v) => {
                // Exit at top level becomes a value
                computed.insert(name.clone(), v);
            }
            TransformResult::Error(e) => {
                return Err((name.clone(), e));
            }
        }
    }

    Ok(computed)
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use dataverse_lib::model::Entity;

    use crate::apps::migration::engine::PathCache;
    use crate::apps::migration::engine::StubFindCache;
    use crate::apps::migration::types::TransformData;

    use super::*;

    fn test_system_vars() -> SystemVars {
        SystemVars::new(Entity::logical("account"), Entity::logical("contact"), 0)
    }

    fn empty_path_cache() -> PathCache {
        PathCache::new()
    }

    #[test]
    fn compute_single_constant_variable() {
        let variables = vec![(
            "prefix".to_string(),
            vec![ChainItem::new(TransformData::Constant {
                value: Value::String("ACME".to_string()),
            })],
        )];

        let source = Record::new("account");
        let cache = StubFindCache;
        let sys = test_system_vars();
        let pc = empty_path_cache();

        let result = compute_variables(&variables, &source, &sys, &cache, &pc).unwrap();
        assert_eq!(
            result.get("prefix"),
            Some(&Value::String("ACME".to_string()))
        );
    }

    #[test]
    fn later_variable_sees_earlier() {
        // $first = "hello"
        // $second = copy from source field "name" (which we'll set to "world")
        // We can't easily test cross-variable reference without the format transform,
        // but we can verify both are computed and present.
        let variables = vec![
            (
                "first".to_string(),
                vec![ChainItem::new(TransformData::Constant {
                    value: Value::String("hello".to_string()),
                })],
            ),
            (
                "second".to_string(),
                vec![ChainItem::new(TransformData::Copy {
                    path: "name".to_string(),
                })],
            ),
        ];

        let source = Record::new("account").set("name", "world");
        let cache = StubFindCache;
        let sys = test_system_vars();
        let pc = empty_path_cache();

        let result = compute_variables(&variables, &source, &sys, &cache, &pc).unwrap();
        assert_eq!(
            result.get("first"),
            Some(&Value::String("hello".to_string()))
        );
        assert_eq!(
            result.get("second"),
            Some(&Value::String("world".to_string()))
        );
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn error_in_variable_returns_name() {
        let variables = vec![(
            "bad_var".to_string(),
            vec![ChainItem::new(TransformData::Copy {
                path: "nonexistent".to_string(),
            })],
        )];

        let source = Record::new("account");
        let cache = StubFindCache;
        let sys = test_system_vars();
        let pc = empty_path_cache();

        let err = compute_variables(&variables, &source, &sys, &cache, &pc).unwrap_err();
        assert_eq!(err.0, "bad_var");
    }

    #[test]
    fn empty_variables_returns_empty_map() {
        let source = Record::new("account");
        let cache = StubFindCache;
        let sys = test_system_vars();
        let pc = empty_path_cache();

        let result = compute_variables(&[], &source, &sys, &cache, &pc).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn variable_chain_starts_with_null() {
        // A chain with no copy/constant starts with #value = Null.
        // StringOps passes through Null, so the variable ends up as Null.
        let variables = vec![(
            "starts_null".to_string(),
            vec![ChainItem::new(TransformData::StringOps {
                op: crate::apps::migration::types::StringOp::Uppercase,
            })],
        )];

        let source = Record::new("account");
        let cache = StubFindCache;
        let sys = test_system_vars();
        let pc = empty_path_cache();

        let result = compute_variables(&variables, &source, &sys, &cache, &pc).unwrap();
        assert_eq!(result.get("starts_null"), Some(&Value::Null));
    }
}
