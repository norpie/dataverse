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
                let value = resolve_placeholder(&placeholder, ctx)?;
                let formatted = format_value(&value);
                result.push_str(&formatted.display);
            }
        } else {
            result.push(ch);
        }
    }

    Ok(result)
}

/// Resolve a placeholder, supporting coalesce syntax (`??`).
///
/// `{a ?? b ?? c}` tries each path left-to-right and returns the first non-null value.
/// If all alternatives resolve to null, returns `Value::Null`.
/// If any alternative fails with an error other than null/not-found, that error propagates.
fn resolve_placeholder(
    placeholder: &str,
    ctx: &ResolveContext<'_>,
) -> Result<Value, TransformError> {
    if !placeholder.contains("??") {
        // Fast path: no coalesce
        let (resolve_result, _) = resolve_path_str(placeholder, ctx);
        return match resolve_result {
            TransformResult::Value(v) => Ok(v),
            TransformResult::Exit(v) => Ok(v),
            TransformResult::Error(e) => Err(e),
        };
    }

    let alternatives: Vec<&str> = placeholder.split("??").map(|s| s.trim()).collect();
    let mut last_error = None;

    for alt in &alternatives {
        if alt.is_empty() {
            continue;
        }
        let (resolve_result, _) = resolve_path_str(alt, ctx);
        match resolve_result {
            TransformResult::Value(Value::Null) | TransformResult::Exit(Value::Null) => continue,
            TransformResult::Value(v) => return Ok(v),
            TransformResult::Exit(v) => return Ok(v),
            TransformResult::Error(
                TransformError::PathNotFound { .. }
                | TransformError::NullInPath { .. }
                | TransformError::VariableNotFound { .. },
            ) => {
                last_error = Some(resolve_result);
                continue;
            }
            TransformResult::Error(e) => return Err(e),
        }
    }

    // All alternatives were null or not-found — return Null (like coalesce semantics)
    let _ = last_error;
    Ok(Value::Null)
}

/// Extract raw placeholder strings from a format template.
///
/// Returns each non-empty string found between `{` and `}`.
/// Unclosed braces and empty `{}` are skipped.
pub fn extract_placeholders(template: &str) -> Vec<String> {
    let mut placeholders = Vec::new();
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
                placeholders.push(placeholder);
            }
        }
    }

    placeholders
}

/// Split a placeholder into individual path expressions, handling coalesce (`??`) syntax.
///
/// `"a ?? b ?? c"` → `["a", "b", "c"]`
/// `"a"` → `["a"]`
pub fn split_coalesce(placeholder: &str) -> Vec<&str> {
    if placeholder.contains("??") {
        placeholder
            .split("??")
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect()
    } else {
        vec![placeholder]
    }
}

/// Extract all field paths from a format template.
///
/// Used for field requirement extraction to build `$expand`.
/// Only returns field paths (not variables or system vars).
pub fn extract_field_paths(template: &str) -> Vec<FieldPath> {
    let mut paths = Vec::new();

    for placeholder in extract_placeholders(template) {
        for alt in split_coalesce(&placeholder) {
            if let Ok(PathExpr::Field(field_path)) = parse_path(alt) {
                paths.push(field_path);
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

    // === Coalesce tests ===

    #[test]
    fn coalesce_returns_first_non_null() {
        let record = Record::new("account")
            .set("email1", Value::Null)
            .set("email2", "second@example.com")
            .set("email3", "third@example.com");
        let vars = HashMap::new();
        let value = Value::Null;
        let ctx = make_ctx(&record, &vars, &value);

        let result = execute_format("{email1 ?? email2 ?? email3}", &ctx);
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

        let result = execute_format("{email1 ?? email2}", &ctx);
        assert!(
            matches!(result, TransformResult::Value(Value::String(s)) if s == "first@example.com")
        );
    }

    #[test]
    fn coalesce_all_null_returns_empty() {
        let record = Record::new("account")
            .set("email1", Value::Null)
            .set("email2", Value::Null);
        let vars = HashMap::new();
        let value = Value::Null;
        let ctx = make_ctx(&record, &vars, &value);

        // All null → Value::Null → formats as ""
        let result = execute_format("{email1 ?? email2}", &ctx);
        assert!(matches!(result, TransformResult::Value(Value::String(s)) if s.is_empty()));
    }

    #[test]
    fn coalesce_missing_field_skipped() {
        let record = Record::new("account").set("email2", "found@example.com");
        let vars = HashMap::new();
        let value = Value::Null;
        let ctx = make_ctx(&record, &vars, &value);

        // nonexistent is PathNotFound → skipped, email2 resolves
        let result = execute_format("{nonexistent ?? email2}", &ctx);
        assert!(
            matches!(result, TransformResult::Value(Value::String(s)) if s == "found@example.com")
        );
    }

    #[test]
    fn coalesce_with_variables() {
        let record = Record::new("account").set("email", Value::Null);
        let mut vars = HashMap::new();
        vars.insert(
            "fallback".to_string(),
            Value::String("var@example.com".to_string()),
        );
        let value = Value::Null;
        let ctx = make_ctx(&record, &vars, &value);

        let result = execute_format("{email ?? $fallback}", &ctx);
        assert!(
            matches!(result, TransformResult::Value(Value::String(s)) if s == "var@example.com")
        );
    }

    #[test]
    fn coalesce_in_template_with_text() {
        let record = Record::new("account")
            .set("email1", Value::Null)
            .set("email2", "found@example.com");
        let vars = HashMap::new();
        let value = Value::Null;
        let ctx = make_ctx(&record, &vars, &value);

        let result = execute_format("Contact: {email1 ?? email2}", &ctx);
        assert!(
            matches!(result, TransformResult::Value(Value::String(s)) if s == "Contact: found@example.com")
        );
    }

    #[test]
    fn extract_field_paths_with_coalesce() {
        let paths = extract_field_paths("{email1 ?? email2 ?? $fallback}");

        // Only field paths, not variables
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0].segments[0].field, "email1");
        assert_eq!(paths[1].segments[0].field, "email2");
    }
}
