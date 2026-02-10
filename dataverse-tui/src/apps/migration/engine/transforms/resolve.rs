//! Shared path resolution for transforms.
//!
//! Resolves a parsed `PathExpr` to a concrete value using the transform context.
//! Used by both `copy` and `format` transforms.

use std::collections::HashMap;

use dataverse_lib::model::Entity;
use dataverse_lib::model::Record;
use dataverse_lib::model::Value;

use crate::apps::migration::engine::TransformError;
use crate::apps::migration::engine::TransformResult;
use crate::apps::migration::types::SystemVar;
use crate::apps::migration::validation::FieldPath;
use crate::apps::migration::validation::PathExpr;

/// Context needed for path resolution.
///
/// A subset of `TransformContext` to avoid coupling to the full execution context.
pub struct ResolveContext<'a> {
    /// The source record being transformed.
    pub source_record: &'a Record,
    /// Computed variables (keyed by name without $ prefix).
    pub variables: &'a HashMap<String, Value>,
    /// Current value in the transform chain (`#value`).
    pub value: &'a Value,
    /// Type annotation of current value (`#type`).
    pub value_type: &'a Option<Entity>,
    /// Record index in the current batch (`#index`).
    pub index: usize,
    /// Source entity (`#source_entity`).
    pub source_entity: Entity,
    /// Target entity (`#target_entity`).
    pub target_entity: Entity,
}

/// Resolve a parsed path expression to a value.
///
/// Returns `(result, entity)` where `entity` is the type annotation from
/// the last traversed lookup (used by `copy` to set `#type`).
pub fn resolve_path(
    path: &PathExpr,
    ctx: &ResolveContext<'_>,
) -> (TransformResult, Option<Entity>) {
    match path {
        PathExpr::Field(field_path) => traverse_record(ctx.source_record, field_path, None),

        PathExpr::Variable(name) => match ctx.variables.get(name.as_str()) {
            Some(v) => (TransformResult::Value(v.clone()), None),
            None => (
                TransformResult::Error(TransformError::variable_not_found(name)),
                None,
            ),
        },

        PathExpr::VariableNavigation {
            name,
            target: _,
            path,
        } => {
            let var_value = match ctx.variables.get(name.as_str()) {
                Some(v) => v,
                None => {
                    return (
                        TransformResult::Error(TransformError::variable_not_found(name)),
                        None,
                    );
                }
            };

            match var_value {
                Value::Record(record) => {
                    let initial_entity = Some(record.entity().clone());
                    traverse_record(record, path, initial_entity)
                }
                Value::Null => (
                    TransformResult::Error(TransformError::type_mismatch("Record", "Null")),
                    None,
                ),
                other => (
                    TransformResult::Error(TransformError::type_mismatch(
                        "Record",
                        format!("{other:?}"),
                    )),
                    None,
                ),
            }
        }

        PathExpr::SystemVar(var) => {
            let value = match var {
                SystemVar::Value => ctx.value.clone(),
                SystemVar::Type => match ctx.value_type {
                    Some(entity) => Value::String(entity.name().to_string()),
                    None => Value::Null,
                },
                SystemVar::Index => Value::Int(ctx.index as i32),
                SystemVar::SourceEntity => Value::String(ctx.source_entity.name().to_string()),
                SystemVar::TargetEntity => Value::String(ctx.target_entity.name().to_string()),
            };
            (TransformResult::Value(value), None)
        }
    }
}

/// Traverse a record following a field path.
///
/// `initial_entity` is set when starting from a variable's Record (so we
/// already know the entity type of the root).
fn traverse_record(
    root: &Record,
    field_path: &FieldPath,
    initial_entity: Option<Entity>,
) -> (TransformResult, Option<Entity>) {
    if field_path.segments.is_empty() {
        return (
            TransformResult::Error(TransformError::path_not_found("<empty>")),
            None,
        );
    }

    let mut current_record = root;
    let mut last_entity = initial_entity;

    // Traverse through lookup segments (all except last)
    let lookups = &field_path.segments[..field_path.segments.len() - 1];
    for segment in lookups {
        match current_record.get(&segment.field) {
            Some(Value::Record(nested)) => {
                last_entity = Some(nested.entity().clone());
                current_record = nested;
            }
            Some(Value::Null) => {
                if segment.optional {
                    return (TransformResult::Value(Value::Null), None);
                } else {
                    return (
                        TransformResult::Error(TransformError::null_in_path(&segment.field)),
                        None,
                    );
                }
            }
            Some(_) | None => {
                return (
                    TransformResult::Error(TransformError::path_not_found(&segment.field)),
                    None,
                );
            }
        }
    }

    // Get the leaf value
    let leaf = &field_path.segments[field_path.segments.len() - 1];
    match current_record.get(&leaf.field) {
        Some(value) => (TransformResult::Value(value.clone()), last_entity),
        None => (
            TransformResult::Error(TransformError::path_not_found(&leaf.field)),
            None,
        ),
    }
}

/// Convenience: resolve a raw path string.
///
/// Parses the path and resolves it. Returns a parse error if the path is invalid.
pub fn resolve_path_str(path: &str, ctx: &ResolveContext<'_>) -> (TransformResult, Option<Entity>) {
    match crate::apps::migration::validation::parse_path(path) {
        Ok(parsed) => resolve_path(&parsed, ctx),
        Err(e) => (
            TransformResult::Error(TransformError::path_not_found(format!("{}: {}", path, e))),
            None,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_source_record() -> Record {
        let grandparent = Record::new("account")
            .set("name", "Parent Corp")
            .set("accountid", "grand-guid");

        let contact = Record::new("contact")
            .set("fullname", "John Smith")
            .set("parentcustomerid", Value::Record(Box::new(grandparent)));

        Record::new("account")
            .set("name", "Contoso")
            .set("revenue", 1_000_000i64)
            .set("primarycontactid", Value::Record(Box::new(contact)))
            .set("secondarycontactid", Value::Null)
    }

    fn make_variables() -> HashMap<String, Value> {
        let capacity = Record::new("capacity")
            .set("capacityid", "cap-guid-123")
            .set("name", "Standard");

        let nested_account = Record::new("account")
            .set("name", "Nested Corp")
            .set("accountid", "nested-guid");

        let found_contact = Record::new("contact")
            .set("fullname", "Found Person")
            .set("parentcustomerid", Value::Record(Box::new(nested_account)));

        let mut vars = HashMap::new();
        vars.insert("capacity".to_string(), Value::Record(Box::new(capacity)));
        vars.insert(
            "found_contact".to_string(),
            Value::Record(Box::new(found_contact)),
        );
        vars.insert("prefix".to_string(), Value::String("ACCT".to_string()));
        vars.insert("empty".to_string(), Value::Null);
        vars
    }

    fn make_ctx<'a>(
        source: &'a Record,
        variables: &'a HashMap<String, Value>,
        value: &'a Value,
    ) -> ResolveContext<'a> {
        ResolveContext {
            source_record: source,
            variables,
            value,
            value_type: &None,
            index: 42,
            source_entity: Entity::logical("account"),
            target_entity: Entity::logical("contact"),
        }
    }

    // =========================================================================
    // Field paths (existing behavior)
    // =========================================================================

    #[test]
    fn field_top_level() {
        let source = make_source_record();
        let vars = make_variables();
        let value = Value::Null;
        let ctx = make_ctx(&source, &vars, &value);

        let (result, entity) = resolve_path_str("name", &ctx);
        assert!(matches!(result, TransformResult::Value(Value::String(ref s)) if s == "Contoso"));
        assert!(entity.is_none());
    }

    #[test]
    fn field_through_lookup() {
        let source = make_source_record();
        let vars = make_variables();
        let value = Value::Null;
        let ctx = make_ctx(&source, &vars, &value);

        let (result, entity) = resolve_path_str("primarycontactid.fullname", &ctx);
        assert!(
            matches!(result, TransformResult::Value(Value::String(ref s)) if s == "John Smith")
        );
        assert_eq!(entity, Some(Entity::logical("contact")));
    }

    #[test]
    fn field_null_optional() {
        let source = make_source_record();
        let vars = make_variables();
        let value = Value::Null;
        let ctx = make_ctx(&source, &vars, &value);

        let (result, _) = resolve_path_str("secondarycontactid?.fullname", &ctx);
        assert!(matches!(result, TransformResult::Value(Value::Null)));
    }

    #[test]
    fn field_null_without_optional_errors() {
        let source = make_source_record();
        let vars = make_variables();
        let value = Value::Null;
        let ctx = make_ctx(&source, &vars, &value);

        let (result, _) = resolve_path_str("secondarycontactid.fullname", &ctx);
        assert!(matches!(
            result,
            TransformResult::Error(TransformError::NullInPath { .. })
        ));
    }

    // =========================================================================
    // Variable paths
    // =========================================================================

    #[test]
    fn variable_plain() {
        let source = make_source_record();
        let vars = make_variables();
        let value = Value::Null;
        let ctx = make_ctx(&source, &vars, &value);

        let (result, _) = resolve_path_str("$prefix", &ctx);
        assert!(matches!(result, TransformResult::Value(Value::String(ref s)) if s == "ACCT"));
    }

    #[test]
    fn variable_navigation_top_level_field() {
        let source = make_source_record();
        let vars = make_variables();
        let value = Value::Null;
        let ctx = make_ctx(&source, &vars, &value);

        let (result, entity) = resolve_path_str("$capacity.capacityid", &ctx);
        assert!(
            matches!(result, TransformResult::Value(Value::String(ref s)) if s == "cap-guid-123")
        );
        assert_eq!(entity, Some(Entity::logical("capacity")));
    }

    #[test]
    fn variable_navigation_through_lookup() {
        let source = make_source_record();
        let vars = make_variables();
        let value = Value::Null;
        let ctx = make_ctx(&source, &vars, &value);

        let (result, entity) = resolve_path_str("$found_contact.parentcustomerid.name", &ctx);
        assert!(
            matches!(result, TransformResult::Value(Value::String(ref s)) if s == "Nested Corp")
        );
        assert_eq!(entity, Some(Entity::logical("account")));
    }

    #[test]
    fn variable_not_found() {
        let source = make_source_record();
        let vars = make_variables();
        let value = Value::Null;
        let ctx = make_ctx(&source, &vars, &value);

        let (result, _) = resolve_path_str("$nonexistent", &ctx);
        assert!(matches!(
            result,
            TransformResult::Error(TransformError::VariableNotFound { .. })
        ));
    }

    #[test]
    fn variable_not_record_errors() {
        let source = make_source_record();
        let vars = make_variables();
        let value = Value::Null;
        let ctx = make_ctx(&source, &vars, &value);

        let (result, _) = resolve_path_str("$prefix.something", &ctx);
        assert!(matches!(
            result,
            TransformResult::Error(TransformError::TypeMismatch { .. })
        ));
    }

    #[test]
    fn variable_null_errors() {
        let source = make_source_record();
        let vars = make_variables();
        let value = Value::Null;
        let ctx = make_ctx(&source, &vars, &value);

        let (result, _) = resolve_path_str("$empty.something", &ctx);
        assert!(matches!(
            result,
            TransformResult::Error(TransformError::TypeMismatch { .. })
        ));
    }

    // =========================================================================
    // System variables
    // =========================================================================

    #[test]
    fn system_var_value() {
        let source = make_source_record();
        let vars = make_variables();
        let value = Value::String("current".to_string());
        let ctx = make_ctx(&source, &vars, &value);

        let (result, _) = resolve_path_str("#value", &ctx);
        assert!(matches!(result, TransformResult::Value(Value::String(ref s)) if s == "current"));
    }

    #[test]
    fn system_var_index() {
        let source = make_source_record();
        let vars = make_variables();
        let value = Value::Null;
        let ctx = make_ctx(&source, &vars, &value);

        let (result, _) = resolve_path_str("#index", &ctx);
        assert!(matches!(result, TransformResult::Value(Value::Int(42))));
    }

    #[test]
    fn system_var_source_entity() {
        let source = make_source_record();
        let vars = make_variables();
        let value = Value::Null;
        let ctx = make_ctx(&source, &vars, &value);

        let (result, _) = resolve_path_str("#source_entity", &ctx);
        assert!(matches!(result, TransformResult::Value(Value::String(ref s)) if s == "account"));
    }

    #[test]
    fn system_var_target_entity() {
        let source = make_source_record();
        let vars = make_variables();
        let value = Value::Null;
        let ctx = make_ctx(&source, &vars, &value);

        let (result, _) = resolve_path_str("#target_entity", &ctx);
        assert!(matches!(result, TransformResult::Value(Value::String(ref s)) if s == "contact"));
    }

    // =========================================================================
    // Parse errors
    // =========================================================================

    #[test]
    fn invalid_path_returns_error() {
        let source = make_source_record();
        let vars = make_variables();
        let value = Value::Null;
        let ctx = make_ctx(&source, &vars, &value);

        let (result, _) = resolve_path_str("$", &ctx);
        assert!(matches!(
            result,
            TransformResult::Error(TransformError::PathNotFound { .. })
        ));
    }
}
