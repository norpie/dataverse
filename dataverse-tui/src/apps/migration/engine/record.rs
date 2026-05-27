//! Record executor — per-record transform execution.
//!
//! Composes variables and field mappings into a complete record transformation.
//! Errors are collected per-field rather than failing the entire record.

use std::collections::HashMap;

use dataverse_lib::model::Record;
use dataverse_lib::model::Value;

use super::executor::ChainItem;
use super::executor::execute_chain;
use super::types::FindCache;
use super::types::PathCache;
use super::types::SystemVars;
use super::types::TransformContext;
use super::types::TransformError;
use super::types::TransformResult;
use super::variables::compute_variables;

// =============================================================================
// Result Type
// =============================================================================

/// Result of executing all transforms for a single source record.
#[derive(Debug, Clone)]
pub struct RecordResult {
    /// Successfully computed target field values.
    pub fields: HashMap<String, Value>,
    /// Per-field errors (field name, error).
    pub errors: Vec<(String, TransformError)>,
    /// Whether the record was intentionally skipped (e.g. Lua script returned
    /// no output for this record). When true, the comparison engine emits
    /// `IgnoreSource` without consulting matching or `NoMatchFallback`.
    pub skipped: bool,
}

impl RecordResult {
    /// Returns true if all field mappings succeeded.
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }

    /// Returns the number of successfully computed fields.
    pub fn field_count(&self) -> usize {
        self.fields.len()
    }

    /// Returns the number of errors.
    pub fn error_count(&self) -> usize {
        self.errors.len()
    }
}

// =============================================================================
// Execution
// =============================================================================

/// Execute all transforms for a single source record.
///
/// 1. Computes variables in order (each can reference earlier ones).
/// 2. Executes each field mapping chain with the computed variables.
/// 3. Collects results and errors per-field.
///
/// # Parameters
///
/// - `source`: The source record being transformed.
/// - `variables`: Variable definitions in computation order: `(name, chain)`.
/// - `field_mappings`: Field mapping definitions: `(target_field, chain)`.
/// - `system_vars`: System variables (index, source/target entity).
/// - `find_cache`: Target data cache for find() resolution.
/// - `path_cache`: Pre-parsed path expressions for this mapping.
pub fn execute_record(
    source: &Record,
    variables: &[(String, Vec<ChainItem>)],
    field_mappings: &[(String, Vec<ChainItem>)],
    system_vars: SystemVars,
    find_cache: &dyn FindCache,
    path_cache: &PathCache,
) -> RecordResult {
    // Step 1: Compute variables
    let computed_vars =
        match compute_variables(variables, source, &system_vars, find_cache, path_cache) {
            Ok(vars) => vars,
            Err((var_name, error)) => {
                // Variable computation failed — return a single error and no fields
                return RecordResult {
                    fields: HashMap::new(),
                    errors: vec![(format!("${}", var_name), error)],
                    skipped: false,
                };
            }
        };

    // Step 2: Execute field mappings
    let mut fields = HashMap::new();
    let mut errors = Vec::new();

    for (target_field, chain) in field_mappings {
        let mut ctx = TransformContext {
            source_record: source,
            variables: &computed_vars,
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
                fields.insert(target_field.clone(), v);
            }
            TransformResult::Exit(v) => {
                // Exit at top level becomes a value
                fields.insert(target_field.clone(), v);
            }
            TransformResult::Error(e) => {
                errors.push((target_field.clone(), e));
            }
        }
    }

    RecordResult {
        fields,
        errors,
        skipped: false,
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use dataverse_lib::model::Entity;

    use crate::apps::migration::engine::StubFindCache;
    use crate::apps::migration::types::CompareOp;
    use crate::apps::migration::types::Condition;
    use crate::apps::migration::types::Expr;
    use crate::apps::migration::types::StringOp;
    use crate::apps::migration::types::TransformData;

    use super::*;

    fn test_system_vars() -> SystemVars {
        SystemVars::new(Entity::logical("account"), Entity::logical("account"), 0)
    }

    fn empty_path_cache() -> PathCache {
        PathCache::new()
    }

    #[test]
    fn simple_copy_mapping() {
        let source = Record::new("account")
            .set("name", "Contoso")
            .set("revenue", 1000);

        let field_mappings = vec![
            (
                "name".to_string(),
                vec![ChainItem::new(TransformData::Copy {
                    path: "name".to_string(),
                })],
            ),
            (
                "annualrevenue".to_string(),
                vec![ChainItem::new(TransformData::Copy {
                    path: "revenue".to_string(),
                })],
            ),
        ];

        let cache = StubFindCache;
        let pc = empty_path_cache();
        let result = execute_record(
            &source,
            &[],
            &field_mappings,
            test_system_vars(),
            &cache,
            &pc,
        );

        assert!(result.is_ok());
        assert_eq!(result.field_count(), 2);
        assert_eq!(
            result.fields.get("name"),
            Some(&Value::String("Contoso".to_string()))
        );
        assert_eq!(result.fields.get("annualrevenue"), Some(&Value::Int(1000)));
    }

    #[test]
    fn copy_with_transform_chain() {
        let source = Record::new("account").set("name", "  contoso  ");

        let field_mappings = vec![(
            "name".to_string(),
            vec![
                ChainItem::new(TransformData::Copy {
                    path: "name".to_string(),
                }),
                ChainItem::new(TransformData::StringOps { op: StringOp::Trim }),
                ChainItem::new(TransformData::StringOps {
                    op: StringOp::Uppercase,
                }),
            ],
        )];

        let cache = StubFindCache;
        let pc = empty_path_cache();
        let result = execute_record(
            &source,
            &[],
            &field_mappings,
            test_system_vars(),
            &cache,
            &pc,
        );

        assert!(result.is_ok());
        assert_eq!(
            result.fields.get("name"),
            Some(&Value::String("CONTOSO".to_string()))
        );
    }

    #[test]
    fn variables_used_in_field_mapping() {
        let source = Record::new("account").set("name", "Contoso");

        let variables = vec![(
            "prefix".to_string(),
            vec![ChainItem::new(TransformData::Constant {
                value: Value::String("ACME".to_string()),
            })],
        )];

        let field_mappings = vec![(
            "description".to_string(),
            vec![ChainItem::new(TransformData::Format {
                template: "{$prefix} - {name}".to_string(),
            })],
        )];

        let cache = StubFindCache;
        let pc = empty_path_cache();
        let result = execute_record(
            &source,
            &variables,
            &field_mappings,
            test_system_vars(),
            &cache,
            &pc,
        );

        assert!(result.is_ok());
        assert_eq!(
            result.fields.get("description"),
            Some(&Value::String("ACME - Contoso".to_string()))
        );
    }

    #[test]
    fn partial_errors_collected() {
        let source = Record::new("account").set("name", "Contoso");

        let field_mappings = vec![
            (
                "name".to_string(),
                vec![ChainItem::new(TransformData::Copy {
                    path: "name".to_string(),
                })],
            ),
            (
                "bad_field".to_string(),
                vec![ChainItem::new(TransformData::Copy {
                    path: "nonexistent".to_string(),
                })],
            ),
            (
                "guid_field".to_string(),
                vec![ChainItem::new(TransformData::Guid)],
            ),
        ];

        let cache = StubFindCache;
        let pc = empty_path_cache();
        let result = execute_record(
            &source,
            &[],
            &field_mappings,
            test_system_vars(),
            &cache,
            &pc,
        );

        assert!(!result.is_ok());
        assert_eq!(result.field_count(), 2); // name + guid_field succeeded
        assert_eq!(result.error_count(), 1); // bad_field failed
        assert_eq!(result.errors[0].0, "bad_field");
    }

    #[test]
    fn variable_error_stops_everything() {
        let source = Record::new("account");

        let variables = vec![(
            "bad_var".to_string(),
            vec![ChainItem::new(TransformData::Copy {
                path: "nonexistent".to_string(),
            })],
        )];

        let field_mappings = vec![(
            "name".to_string(),
            vec![ChainItem::new(TransformData::Constant {
                value: Value::String("test".to_string()),
            })],
        )];

        let cache = StubFindCache;
        let pc = empty_path_cache();
        let result = execute_record(
            &source,
            &variables,
            &field_mappings,
            test_system_vars(),
            &cache,
            &pc,
        );

        assert!(!result.is_ok());
        assert_eq!(result.field_count(), 0);
        assert_eq!(result.error_count(), 1);
        assert_eq!(result.errors[0].0, "$bad_var");
    }

    #[test]
    fn guard_in_field_mapping() {
        let source = Record::new("account")
            .set("name", "Contoso")
            .set("status", 2);

        // Guard: if status == 1, exit with "Active", otherwise continue to copy name
        let field_mappings = vec![(
            "description".to_string(),
            vec![
                ChainItem::with_fallback(
                    TransformData::Guard {
                        condition: Condition::Compare {
                            left: Expr::Path("status".to_string()),
                            op: CompareOp::Equal,
                            right: Expr::Literal(Value::Int(1)),
                        },
                    },
                    vec![ChainItem::new(TransformData::Constant {
                        value: Value::String("Active".to_string()),
                    })],
                ),
                ChainItem::new(TransformData::Copy {
                    path: "name".to_string(),
                }),
            ],
        )];

        let cache = StubFindCache;
        let pc = empty_path_cache();
        let result = execute_record(
            &source,
            &[],
            &field_mappings,
            test_system_vars(),
            &cache,
            &pc,
        );

        assert!(result.is_ok());
        // status != 1, guard doesn't trigger, continues to copy name
        assert_eq!(
            result.fields.get("description"),
            Some(&Value::String("Contoso".to_string()))
        );
    }

    #[test]
    fn guard_triggers_exit() {
        let source = Record::new("account")
            .set("name", "Contoso")
            .set("status", 1);

        // Guard: if status == 1, exit with "Active"
        let field_mappings = vec![(
            "description".to_string(),
            vec![
                ChainItem::with_fallback(
                    TransformData::Guard {
                        condition: Condition::Compare {
                            left: Expr::Path("status".to_string()),
                            op: CompareOp::Equal,
                            right: Expr::Literal(Value::Int(1)),
                        },
                    },
                    vec![ChainItem::new(TransformData::Constant {
                        value: Value::String("Active".to_string()),
                    })],
                ),
                // This should NOT be reached
                ChainItem::new(TransformData::Copy {
                    path: "name".to_string(),
                }),
            ],
        )];

        let cache = StubFindCache;
        let pc = empty_path_cache();
        let result = execute_record(
            &source,
            &[],
            &field_mappings,
            test_system_vars(),
            &cache,
            &pc,
        );

        assert!(result.is_ok());
        // Guard triggered, exit with "Active", skipped copy
        assert_eq!(
            result.fields.get("description"),
            Some(&Value::String("Active".to_string()))
        );
    }

    #[test]
    fn empty_mappings() {
        let source = Record::new("account");
        let cache = StubFindCache;
        let pc = empty_path_cache();
        let result = execute_record(&source, &[], &[], test_system_vars(), &cache, &pc);

        assert!(result.is_ok());
        assert_eq!(result.field_count(), 0);
        assert_eq!(result.error_count(), 0);
    }
}
