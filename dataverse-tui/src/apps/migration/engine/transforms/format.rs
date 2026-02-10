//! Format transform - template string interpolation.

use dataverse_lib::model::Value;

use super::resolve::resolve_path_str;
use super::resolve::ResolveContext;
use crate::apps::migration::engine::TransformError;
use crate::apps::migration::engine::TransformResult;
use crate::apps::migration::validation::parse_path;
use crate::apps::migration::validation::FieldPath;
use crate::apps::migration::validation::PathExpr;
use crate::formatting::format_value;

/// Execute the format transform.
///
/// Interpolates placeholders in a template string:
/// - `{field}` or `{field.path}` - resolved from source record
/// - `{$var}` or `{$var.field}` - resolved from variables
/// - `{#value}`, `{#index}`, etc. - resolved from system variables
///
/// # Examples
///
/// ```ignore
/// // Template: "Hello, {firstname} {lastname}!"
/// // Source: { "firstname": "John", "lastname": "Doe" }
/// // Result: "Hello, John Doe!"
///
/// // Template: "{$prefix}_{accountnumber}"
/// // Variables: { "prefix": "ACCT" }
/// // Source: { "accountnumber": "12345" }
/// // Result: "ACCT_12345"
///
/// // Template: "{$capacity.name} - {name}"
/// // Variables: { "capacity": Record(capacity) }
/// // Source: { "name": "Contoso" }
/// // Result: "Standard - Contoso"
/// ```
pub fn execute_format(template: &str, ctx: &ResolveContext<'_>) -> TransformResult {
    match interpolate(template, ctx) {
        Ok(result) => TransformResult::Value(Value::String(result)),
        Err(e) => TransformResult::Error(e),
    }
}

/// Interpolate placeholders in a template string.
fn interpolate(template: &str, ctx: &ResolveContext<'_>) -> Result<String, TransformError> {
    let mut result = String::with_capacity(template.len());
    let mut chars = template.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '{' {
            // Start of placeholder
            let mut placeholder = String::new();
            let mut found_close = false;

            for inner in chars.by_ref() {
                if inner == '}' {
                    found_close = true;
                    break;
                }
                placeholder.push(inner);
            }

            if !found_close {
                // Unclosed brace - treat as literal
                result.push('{');
                result.push_str(&placeholder);
            } else if placeholder.is_empty() {
                // Empty placeholder {} - treat as literal
                result.push_str("{}");
            } else {
                // Resolve the placeholder using shared path resolution
                let (resolve_result, _) = resolve_path_str(&placeholder, ctx);
                match resolve_result {
                    TransformResult::Value(value) => {
                        let formatted = format_value(&value);
                        result.push_str(&formatted.display);
                    }
                    TransformResult::Error(e) => return Err(e),
                    TransformResult::Exit(value) => {
                        let formatted = format_value(&value);
                        result.push_str(&formatted.display);
                    }
                }
            }
        } else {
            result.push(ch);
        }
    }

    Ok(result)
}

/// Extract all field paths from a format template.
///
/// Used for field requirement extraction to build `$expand`.
/// Only returns field paths (not variables or system vars).
pub fn extract_field_paths(template: &str) -> Vec<FieldPath> {
    let mut paths = Vec::new();
    let mut chars = template.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '{' {
            let mut placeholder = String::new();
            let mut found_close = false;

            for inner in chars.by_ref() {
                if inner == '}' {
                    found_close = true;
                    break;
                }
                placeholder.push(inner);
            }

            if found_close && !placeholder.is_empty() {
                // Only extract field paths (not variables or system vars)
                if let Ok(PathExpr::Field(field_path)) = parse_path(&placeholder) {
                    paths.push(field_path);
                }
            }
        }
    }

    paths
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use dataverse_lib::model::Entity;
    use dataverse_lib::model::Record;

    use super::*;

    fn make_record() -> Record {
        let contact = Record::new("contact")
            .set("fullname", "John Smith")
            .set("email", "john@example.com");

        Record::new("account")
            .set("name", "Contoso")
            .set("accountnumber", "12345")
            .set("primarycontactid", Value::Record(Box::new(contact)))
            .set("secondarycontactid", Value::Null)
    }

    fn make_variables() -> HashMap<String, Value> {
        let capacity = Record::new("capacity")
            .set("name", "Standard")
            .set("capacityid", "cap-123");

        let mut vars = HashMap::new();
        vars.insert("prefix".to_string(), Value::String("ACCT".to_string()));
        vars.insert("suffix".to_string(), Value::String("INC".to_string()));
        vars.insert("capacity".to_string(), Value::Record(Box::new(capacity)));
        vars
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
            index: 5,
            source_entity: Entity::logical("account"),
            target_entity: Entity::logical("contact"),
        }
    }

    #[test]
    fn simple_field_interpolation() {
        let record = make_record();
        let vars = HashMap::new();
        let value = Value::Null;
        let ctx = make_ctx(&record, &vars, &value);

        let result = execute_format("Hello, {name}!", &ctx);
        assert!(
            matches!(result, TransformResult::Value(Value::String(s)) if s == "Hello, Contoso!")
        );
    }

    #[test]
    fn multiple_fields() {
        let record = make_record();
        let vars = HashMap::new();
        let value = Value::Null;
        let ctx = make_ctx(&record, &vars, &value);

        let result = execute_format("{name} - {accountnumber}", &ctx);
        assert!(
            matches!(result, TransformResult::Value(Value::String(s)) if s == "Contoso - 12345")
        );
    }

    #[test]
    fn nested_field_path() {
        let record = make_record();
        let vars = HashMap::new();
        let value = Value::Null;
        let ctx = make_ctx(&record, &vars, &value);

        let result = execute_format("Contact: {primarycontactid.fullname}", &ctx);
        assert!(
            matches!(result, TransformResult::Value(Value::String(s)) if s == "Contact: John Smith")
        );
    }

    #[test]
    fn variable_interpolation() {
        let record = make_record();
        let vars = make_variables();
        let value = Value::Null;
        let ctx = make_ctx(&record, &vars, &value);

        let result = execute_format("{$prefix}_{accountnumber}", &ctx);
        assert!(matches!(result, TransformResult::Value(Value::String(s)) if s == "ACCT_12345"));
    }

    #[test]
    fn mixed_fields_and_variables() {
        let record = make_record();
        let vars = make_variables();
        let value = Value::Null;
        let ctx = make_ctx(&record, &vars, &value);

        let result = execute_format("{$prefix}-{name}-{$suffix}", &ctx);
        assert!(
            matches!(result, TransformResult::Value(Value::String(s)) if s == "ACCT-Contoso-INC")
        );
    }

    #[test]
    fn variable_navigation_in_template() {
        let record = make_record();
        let vars = make_variables();
        let value = Value::Null;
        let ctx = make_ctx(&record, &vars, &value);

        let result = execute_format("{$capacity.name} - {name}", &ctx);
        assert!(
            matches!(result, TransformResult::Value(Value::String(s)) if s == "Standard - Contoso")
        );
    }

    #[test]
    fn system_var_in_template() {
        let record = make_record();
        let vars = HashMap::new();
        let value = Value::String("current".to_string());
        let ctx = make_ctx(&record, &vars, &value);

        let result = execute_format("Value: {#value}, Index: {#index}", &ctx);
        assert!(
            matches!(result, TransformResult::Value(Value::String(s)) if s == "Value: current, Index: 5")
        );
    }

    #[test]
    fn missing_field_returns_error() {
        let record = make_record();
        let vars = HashMap::new();
        let value = Value::Null;
        let ctx = make_ctx(&record, &vars, &value);

        let result = execute_format("Value: {nonexistent}", &ctx);
        assert!(matches!(
            result,
            TransformResult::Error(TransformError::PathNotFound { .. })
        ));
    }

    #[test]
    fn missing_variable_returns_error() {
        let record = make_record();
        let vars = HashMap::new();
        let value = Value::Null;
        let ctx = make_ctx(&record, &vars, &value);

        let result = execute_format("Value: {$unknown}", &ctx);
        assert!(matches!(
            result,
            TransformResult::Error(TransformError::VariableNotFound { .. })
        ));
    }

    #[test]
    fn null_lookup_without_optional_returns_error() {
        let record = make_record();
        let vars = HashMap::new();
        let value = Value::Null;
        let ctx = make_ctx(&record, &vars, &value);

        let result = execute_format("Contact: {secondarycontactid.fullname}", &ctx);
        assert!(matches!(
            result,
            TransformResult::Error(TransformError::NullInPath { .. })
        ));
    }

    #[test]
    fn null_lookup_with_optional_returns_empty() {
        let record = make_record();
        let vars = HashMap::new();
        let value = Value::Null;
        let ctx = make_ctx(&record, &vars, &value);

        let result = execute_format("Contact: {secondarycontactid?.fullname}", &ctx);
        // Null formats as empty string
        assert!(matches!(result, TransformResult::Value(Value::String(s)) if s == "Contact: "));
    }

    #[test]
    fn literal_braces_preserved() {
        let record = make_record();
        let vars = HashMap::new();
        let value = Value::Null;
        let ctx = make_ctx(&record, &vars, &value);

        // Unclosed brace
        let result = execute_format("Value: {name", &ctx);
        assert!(matches!(result, TransformResult::Value(Value::String(s)) if s == "Value: {name"));

        // Empty braces
        let result = execute_format("Empty: {}", &ctx);
        assert!(matches!(result, TransformResult::Value(Value::String(s)) if s == "Empty: {}"));
    }

    #[test]
    fn no_placeholders() {
        let record = make_record();
        let vars = HashMap::new();
        let value = Value::Null;
        let ctx = make_ctx(&record, &vars, &value);

        let result = execute_format("Just plain text", &ctx);
        assert!(
            matches!(result, TransformResult::Value(Value::String(s)) if s == "Just plain text")
        );
    }

    #[test]
    fn extract_field_paths_works() {
        let paths = extract_field_paths(
            "Hello {name}, contact: {primarycontactid.fullname}, var: {$prefix}",
        );

        assert_eq!(paths.len(), 2); // Only field paths, not variables
        assert_eq!(paths[0].segments.len(), 1);
        assert_eq!(paths[0].segments[0].field, "name");
        assert_eq!(paths[1].segments.len(), 2);
        assert_eq!(paths[1].segments[0].field, "primarycontactid");
        assert_eq!(paths[1].segments[1].field, "fullname");
    }
}
