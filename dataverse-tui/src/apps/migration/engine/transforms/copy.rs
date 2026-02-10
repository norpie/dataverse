//! Copy transform - copies a value from source record, variables, or system vars.
//!
//! Supports all path types from `parse_path()`:
//! - Field paths: `name`, `primarycontactid.fullname`
//! - Variable access: `$var`, `$var.field`
//! - System variables: `#value`, `#index`, etc.

use dataverse_lib::model::Entity;
use dataverse_lib::model::Value;

use super::format::split_coalesce;
use super::resolve::resolve_path_str;
use super::resolve::ResolveContext;
use crate::apps::migration::engine::TransformError;
use crate::apps::migration::engine::TransformResult;

/// Execute the copy transform.
///
/// Parses the path and resolves it against the context. Supports field paths,
/// variable navigation (`$var.field`), system variables (`#value`), and
/// coalesce syntax (`path1 ?? path2 ?? path3`).
///
/// # Returns
///
/// - `(TransformResult::Value, Some(entity))` - value with type from traversed lookup
/// - `(TransformResult::Value, None)` - value from top-level field or variable
/// - `(TransformResult::Error, None)` - path resolution failed
pub fn execute_copy(path: &str, ctx: &ResolveContext<'_>) -> (TransformResult, Option<Entity>) {
    if !path.contains("??") {
        return resolve_path_str(path, ctx);
    }

    let alternatives = split_coalesce(path);
    let mut last_error = None;

    for alt in &alternatives {
        let (result, entity) = resolve_path_str(alt, ctx);
        match result {
            TransformResult::Value(Value::Null) | TransformResult::Exit(Value::Null) => continue,
            TransformResult::Value(_) | TransformResult::Exit(_) => return (result, entity),
            TransformResult::Error(
                TransformError::PathNotFound { .. }
                | TransformError::NullInPath { .. }
                | TransformError::VariableNotFound { .. },
            ) => {
                last_error = Some(result);
                continue;
            }
            TransformResult::Error(_) => return (result, None),
        }
    }

    let _ = last_error;
    (TransformResult::Value(Value::Null), None)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use dataverse_lib::model::Record;
    use dataverse_lib::model::Value;

    use super::*;
    use crate::apps::migration::engine::TransformError;

    fn make_record() -> Record {
        // account with expanded primarycontactid -> contact with expanded parentcustomerid -> account
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
            .set("secondarycontactid", Value::Null) // null lookup for testing
    }

    fn make_ctx<'a>(
        record: &'a Record,
        vars: &'a HashMap<String, Value>,
        value: &'a Value,
    ) -> ResolveContext<'a> {
        ResolveContext {
            source_record: record,
            variables: vars,
            value,
            value_type: &None,
            index: 0,
            source_entity: Entity::logical("account"),
            target_entity: Entity::logical("contact"),
        }
    }

    #[test]
    fn copies_top_level_field() {
        let record = make_record();
        let vars = HashMap::new();
        let value = Value::Null;
        let ctx = make_ctx(&record, &vars, &value);
        let (result, value_type) = execute_copy("name", &ctx);

        assert!(matches!(result, TransformResult::Value(Value::String(s)) if s == "Contoso"));
        assert!(value_type.is_none()); // No lookup traversal
    }

    #[test]
    fn copies_through_one_lookup() {
        let record = make_record();
        let vars = HashMap::new();
        let value = Value::Null;
        let ctx = make_ctx(&record, &vars, &value);
        let (result, value_type) = execute_copy("primarycontactid.fullname", &ctx);

        assert!(matches!(result, TransformResult::Value(Value::String(s)) if s == "John Smith"));
        assert_eq!(value_type, Some(Entity::logical("contact")));
    }

    #[test]
    fn copies_through_two_lookups() {
        let record = make_record();
        let vars = HashMap::new();
        let value = Value::Null;
        let ctx = make_ctx(&record, &vars, &value);
        let (result, value_type) = execute_copy("primarycontactid.parentcustomerid.name", &ctx);

        assert!(matches!(result, TransformResult::Value(Value::String(s)) if s == "Parent Corp"));
        assert_eq!(value_type, Some(Entity::logical("account")));
    }

    #[test]
    fn missing_field_returns_error() {
        let record = make_record();
        let vars = HashMap::new();
        let value = Value::Null;
        let ctx = make_ctx(&record, &vars, &value);
        let (result, _) = execute_copy("nonexistent", &ctx);

        assert!(matches!(
            result,
            TransformResult::Error(TransformError::PathNotFound { .. })
        ));
    }

    #[test]
    fn missing_nested_field_returns_error() {
        let record = make_record();
        let vars = HashMap::new();
        let value = Value::Null;
        let ctx = make_ctx(&record, &vars, &value);
        let (result, _) = execute_copy("primarycontactid.nonexistent", &ctx);

        assert!(matches!(
            result,
            TransformResult::Error(TransformError::PathNotFound { .. })
        ));
    }

    #[test]
    fn traverse_non_record_returns_error() {
        let record = make_record();
        let vars = HashMap::new();
        let value = Value::Null;
        let ctx = make_ctx(&record, &vars, &value);
        let (result, _) = execute_copy("name.something", &ctx);

        assert!(matches!(
            result,
            TransformResult::Error(TransformError::PathNotFound { .. })
        ));
    }

    #[test]
    fn null_lookup_without_optional_returns_error() {
        let record = make_record();
        let vars = HashMap::new();
        let value = Value::Null;
        let ctx = make_ctx(&record, &vars, &value);
        let (result, _) = execute_copy("secondarycontactid.fullname", &ctx);

        assert!(matches!(
            result,
            TransformResult::Error(TransformError::NullInPath { .. })
        ));
    }

    #[test]
    fn null_lookup_with_optional_returns_null() {
        let record = make_record();
        let vars = HashMap::new();
        let value = Value::Null;
        let ctx = make_ctx(&record, &vars, &value);
        let (result, value_type) = execute_copy("secondarycontactid?.fullname", &ctx);

        assert!(matches!(result, TransformResult::Value(Value::Null)));
        assert!(value_type.is_none());
    }

    #[test]
    fn optional_on_non_null_lookup_works() {
        let record = make_record();
        let vars = HashMap::new();
        let value = Value::Null;
        let ctx = make_ctx(&record, &vars, &value);
        let (result, value_type) = execute_copy("primarycontactid?.fullname", &ctx);

        assert!(matches!(result, TransformResult::Value(Value::String(s)) if s == "John Smith"));
        assert_eq!(value_type, Some(Entity::logical("contact")));
    }

    #[test]
    fn chained_optional_with_null_in_middle() {
        let contact = Record::new("contact")
            .set("fullname", "Jane Doe")
            .set("parentcustomerid", Value::Null);

        let record = Record::new("account")
            .set("name", "Test")
            .set("primarycontactid", Value::Record(Box::new(contact)));

        let vars = HashMap::new();
        let value = Value::Null;
        let ctx = make_ctx(&record, &vars, &value);

        // Without ? on parentcustomerid - should error
        let (result, _) = execute_copy("primarycontactid.parentcustomerid.name", &ctx);
        assert!(matches!(
            result,
            TransformResult::Error(TransformError::NullInPath { .. })
        ));

        // With ? on parentcustomerid - should return null
        let (result, _) = execute_copy("primarycontactid.parentcustomerid?.name", &ctx);
        assert!(matches!(result, TransformResult::Value(Value::Null)));
    }

    // =========================================================================
    // Variable paths (new)
    // =========================================================================

    #[test]
    fn copies_from_variable() {
        let record = make_record();
        let mut vars = HashMap::new();
        vars.insert("prefix".to_string(), Value::String("ACCT".to_string()));
        let value = Value::Null;
        let ctx = make_ctx(&record, &vars, &value);

        let (result, _) = execute_copy("$prefix", &ctx);
        assert!(matches!(result, TransformResult::Value(Value::String(s)) if s == "ACCT"));
    }

    #[test]
    fn copies_field_from_variable_record() {
        let record = make_record();
        let capacity = Record::new("capacity")
            .set("capacityid", "cap-123")
            .set("name", "Standard");
        let mut vars = HashMap::new();
        vars.insert("capacity".to_string(), Value::Record(Box::new(capacity)));
        let value = Value::Null;
        let ctx = make_ctx(&record, &vars, &value);

        let (result, entity) = execute_copy("$capacity.capacityid", &ctx);
        assert!(matches!(result, TransformResult::Value(Value::String(s)) if s == "cap-123"));
        assert_eq!(entity, Some(Entity::logical("capacity")));
    }

    #[test]
    fn copies_system_var_value() {
        let record = make_record();
        let vars = HashMap::new();
        let value = Value::String("current-value".to_string());
        let ctx = make_ctx(&record, &vars, &value);

        let (result, _) = execute_copy("#value", &ctx);
        assert!(matches!(result, TransformResult::Value(Value::String(s)) if s == "current-value"));
    }

    // =========================================================================
    // Coalesce paths
    // =========================================================================

    #[test]
    fn coalesce_returns_first_non_null() {
        let record = Record::new("account")
            .set("email1", Value::Null)
            .set("email2", "second@example.com")
            .set("email3", "third@example.com");
        let vars = HashMap::new();
        let value = Value::Null;
        let ctx = make_ctx(&record, &vars, &value);

        let (result, _) = execute_copy("email1 ?? email2 ?? email3", &ctx);
        assert!(
            matches!(result, TransformResult::Value(Value::String(s)) if s == "second@example.com")
        );
    }

    #[test]
    fn coalesce_returns_first_when_not_null() {
        let record = Record::new("account")
            .set("email1", "first@example.com")
            .set("email2", "second@example.com");
        let vars = HashMap::new();
        let value = Value::Null;
        let ctx = make_ctx(&record, &vars, &value);

        let (result, _) = execute_copy("email1 ?? email2", &ctx);
        assert!(
            matches!(result, TransformResult::Value(Value::String(s)) if s == "first@example.com")
        );
    }

    #[test]
    fn coalesce_all_null_returns_null() {
        let record = Record::new("account")
            .set("email1", Value::Null)
            .set("email2", Value::Null);
        let vars = HashMap::new();
        let value = Value::Null;
        let ctx = make_ctx(&record, &vars, &value);

        let (result, _) = execute_copy("email1 ?? email2", &ctx);
        assert!(matches!(result, TransformResult::Value(Value::Null)));
    }

    #[test]
    fn coalesce_missing_field_skipped() {
        let record = Record::new("account").set("email2", "found@example.com");
        let vars = HashMap::new();
        let value = Value::Null;
        let ctx = make_ctx(&record, &vars, &value);

        let (result, _) = execute_copy("nonexistent ?? email2", &ctx);
        assert!(
            matches!(result, TransformResult::Value(Value::String(s)) if s == "found@example.com")
        );
    }

    #[test]
    fn coalesce_with_variable_fallback() {
        let record = Record::new("account").set("email", Value::Null);
        let mut vars = HashMap::new();
        vars.insert(
            "fallback".to_string(),
            Value::String("var@example.com".to_string()),
        );
        let value = Value::Null;
        let ctx = make_ctx(&record, &vars, &value);

        let (result, _) = execute_copy("email ?? $fallback", &ctx);
        assert!(
            matches!(result, TransformResult::Value(Value::String(s)) if s == "var@example.com")
        );
    }

    #[test]
    fn coalesce_preserves_entity_type() {
        let contact = Record::new("contact").set("fullname", "John");
        let record = Record::new("account")
            .set("primarycontactid", Value::Null)
            .set("secondarycontactid", Value::Record(Box::new(contact)));
        let vars = HashMap::new();
        let value = Value::Null;
        let ctx = make_ctx(&record, &vars, &value);

        let (result, entity) = execute_copy(
            "primarycontactid?.fullname ?? secondarycontactid.fullname",
            &ctx,
        );
        assert!(matches!(result, TransformResult::Value(Value::String(s)) if s == "John"));
        assert_eq!(entity, Some(Entity::logical("contact")));
    }

    #[test]
    fn copies_system_var_index() {
        let record = make_record();
        let vars = HashMap::new();
        let value = Value::Null;
        let ctx = ResolveContext {
            source_record: &record,
            variables: &vars,
            value: &value,
            value_type: &None,
            index: 7,
            source_entity: Entity::logical("account"),
            target_entity: Entity::logical("contact"),
        };

        let (result, _) = execute_copy("#index", &ctx);
        assert!(matches!(result, TransformResult::Value(Value::Int(7))));
    }
}
