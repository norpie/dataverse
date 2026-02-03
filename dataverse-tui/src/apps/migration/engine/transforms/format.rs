//! Format transform - template string interpolation.

use std::collections::HashMap;

use dataverse_lib::model::Record;
use dataverse_lib::model::Value;

use crate::apps::migration::engine::FieldPath;
use crate::apps::migration::engine::TransformError;
use crate::apps::migration::engine::TransformResult;
use crate::formatting::format_value;

/// Execute the format transform.
///
/// Interpolates placeholders in a template string:
/// - `{field}` or `{field.path}` - resolved from source record
/// - `{$var}` - resolved from variables
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
/// ```
pub fn execute_format(
    template: &str,
    source_record: &Record,
    variables: &HashMap<String, Value>,
) -> TransformResult {
    match interpolate(template, source_record, variables) {
        Ok(result) => TransformResult::Value(Value::String(result)),
        Err(e) => TransformResult::Error(e),
    }
}

/// Interpolate placeholders in a template string.
fn interpolate(
    template: &str,
    source_record: &Record,
    variables: &HashMap<String, Value>,
) -> Result<String, TransformError> {
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
                // Resolve the placeholder
                let value = resolve_placeholder(&placeholder, source_record, variables)?;
                let formatted = format_value(&value);
                result.push_str(&formatted.display);
            }
        } else {
            result.push(ch);
        }
    }

    Ok(result)
}

/// Resolve a placeholder to its value.
fn resolve_placeholder(
    placeholder: &str,
    source_record: &Record,
    variables: &HashMap<String, Value>,
) -> Result<Value, TransformError> {
    if let Some(var_name) = placeholder.strip_prefix('$') {
        // Variable reference
        variables
            .get(var_name)
            .cloned()
            .ok_or_else(|| TransformError::variable_not_found(var_name))
    } else {
        // Field path reference
        resolve_field_path(placeholder, source_record)
    }
}

/// Resolve a field path from the source record.
fn resolve_field_path(path: &str, source_record: &Record) -> Result<Value, TransformError> {
    let field_path = FieldPath::parse(path);

    if field_path.is_empty() {
        return Err(TransformError::path_not_found(path));
    }

    let mut current_record = source_record;

    // Traverse through lookups
    for segment in field_path.lookups() {
        match current_record.get(segment.name()) {
            Some(Value::Record(nested)) => {
                current_record = nested;
            }
            Some(Value::Null) => {
                if segment.is_optional() {
                    return Ok(Value::Null);
                } else {
                    return Err(TransformError::null_in_path(segment.name()));
                }
            }
            Some(_) => {
                return Err(TransformError::path_not_found(path));
            }
            None => {
                return Err(TransformError::path_not_found(path));
            }
        }
    }

    // Get the final value
    match field_path.leaf() {
        Some(segment) => current_record
            .get(segment.name())
            .cloned()
            .ok_or_else(|| TransformError::path_not_found(path)),
        None => Err(TransformError::path_not_found(path)),
    }
}

/// Extract all field paths from a format template.
///
/// Used for field requirement extraction to build `$expand`.
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

            if found_close && !placeholder.is_empty() && !placeholder.starts_with('$') {
                // It's a field path, not a variable
                paths.push(FieldPath::parse(&placeholder));
            }
        }
    }

    paths
}

#[cfg(test)]
mod tests {
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
        let mut vars = HashMap::new();
        vars.insert("prefix".to_string(), Value::String("ACCT".to_string()));
        vars.insert("suffix".to_string(), Value::String("INC".to_string()));
        vars
    }

    #[test]
    fn simple_field_interpolation() {
        let record = make_record();
        let vars = HashMap::new();

        let result = execute_format("Hello, {name}!", &record, &vars);
        assert!(
            matches!(result, TransformResult::Value(Value::String(s)) if s == "Hello, Contoso!")
        );
    }

    #[test]
    fn multiple_fields() {
        let record = make_record();
        let vars = HashMap::new();

        let result = execute_format("{name} - {accountnumber}", &record, &vars);
        assert!(
            matches!(result, TransformResult::Value(Value::String(s)) if s == "Contoso - 12345")
        );
    }

    #[test]
    fn nested_field_path() {
        let record = make_record();
        let vars = HashMap::new();

        let result = execute_format("Contact: {primarycontactid.fullname}", &record, &vars);
        assert!(
            matches!(result, TransformResult::Value(Value::String(s)) if s == "Contact: John Smith")
        );
    }

    #[test]
    fn variable_interpolation() {
        let record = make_record();
        let vars = make_variables();

        let result = execute_format("{$prefix}_{accountnumber}", &record, &vars);
        assert!(matches!(result, TransformResult::Value(Value::String(s)) if s == "ACCT_12345"));
    }

    #[test]
    fn mixed_fields_and_variables() {
        let record = make_record();
        let vars = make_variables();

        let result = execute_format("{$prefix}-{name}-{$suffix}", &record, &vars);
        assert!(
            matches!(result, TransformResult::Value(Value::String(s)) if s == "ACCT-Contoso-INC")
        );
    }

    #[test]
    fn missing_field_returns_error() {
        let record = make_record();
        let vars = HashMap::new();

        let result = execute_format("Value: {nonexistent}", &record, &vars);
        assert!(matches!(
            result,
            TransformResult::Error(TransformError::PathNotFound { .. })
        ));
    }

    #[test]
    fn missing_variable_returns_error() {
        let record = make_record();
        let vars = HashMap::new();

        let result = execute_format("Value: {$unknown}", &record, &vars);
        assert!(matches!(
            result,
            TransformResult::Error(TransformError::VariableNotFound { .. })
        ));
    }

    #[test]
    fn null_lookup_without_optional_returns_error() {
        let record = make_record();
        let vars = HashMap::new();

        let result = execute_format("Contact: {secondarycontactid.fullname}", &record, &vars);
        assert!(matches!(
            result,
            TransformResult::Error(TransformError::NullInPath { .. })
        ));
    }

    #[test]
    fn null_lookup_with_optional_returns_empty() {
        let record = make_record();
        let vars = HashMap::new();

        let result = execute_format("Contact: {secondarycontactid?.fullname}", &record, &vars);
        // Null formats as empty string
        assert!(matches!(result, TransformResult::Value(Value::String(s)) if s == "Contact: "));
    }

    #[test]
    fn literal_braces_preserved() {
        let record = make_record();
        let vars = HashMap::new();

        // Unclosed brace
        let result = execute_format("Value: {name", &record, &vars);
        assert!(matches!(result, TransformResult::Value(Value::String(s)) if s == "Value: {name"));

        // Empty braces
        let result = execute_format("Empty: {}", &record, &vars);
        assert!(matches!(result, TransformResult::Value(Value::String(s)) if s == "Empty: {}"));
    }

    #[test]
    fn no_placeholders() {
        let record = make_record();
        let vars = HashMap::new();

        let result = execute_format("Just plain text", &record, &vars);
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
        assert_eq!(paths[0].segments().len(), 1);
        assert_eq!(paths[0].leaf().unwrap().name(), "name");
        assert_eq!(paths[1].segments().len(), 2);
        assert_eq!(paths[1].lookups()[0].name(), "primarycontactid");
        assert_eq!(paths[1].leaf().unwrap().name(), "fullname");
    }
}
