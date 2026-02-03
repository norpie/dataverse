//! Replace transform - string substitution.

use dataverse_lib::model::Value;
use regex::Regex;

use crate::apps::migration::engine::TransformError;
use crate::apps::migration::engine::TransformResult;

/// Execute the replace transform.
///
/// Performs string substitution on `#value`:
/// - If `regex` is false, performs literal string replacement
/// - If `regex` is true, `from` is treated as a regex pattern
///
/// # Examples
///
/// ```ignore
/// // Literal replace
/// // #value = "555-123-4567", from = "-", to = ""
/// // Result: "5551234567"
///
/// // Regex replace
/// // #value = "hello    world", from = r"\s+", to = " "
/// // Result: "hello world"
/// ```
///
/// # Errors
///
/// - `TypeMismatch` if `#value` is not a string
/// - `RegexError` if `regex` is true and pattern is invalid
pub fn execute_replace(value: &Value, from: &str, to: &str, use_regex: bool) -> TransformResult {
    let input = match value {
        Value::String(s) => s,
        Value::Null => return TransformResult::Value(Value::Null),
        other => {
            return TransformResult::Error(TransformError::type_mismatch(
                "string",
                other.type_name(),
            ))
        }
    };

    if use_regex {
        match Regex::new(from) {
            Ok(re) => {
                let result = re.replace_all(input, to).into_owned();
                TransformResult::Value(Value::String(result))
            }
            Err(e) => TransformResult::Error(TransformError::RegexError {
                pattern: from.to_string(),
                message: e.to_string(),
            }),
        }
    } else {
        let result = input.replace(from, to);
        TransformResult::Value(Value::String(result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_replace_single() {
        let value = Value::String("hello-world".to_string());
        let result = execute_replace(&value, "-", "_", false);
        assert!(matches!(result, TransformResult::Value(Value::String(s)) if s == "hello_world"));
    }

    #[test]
    fn literal_replace_multiple() {
        let value = Value::String("555-123-4567".to_string());
        let result = execute_replace(&value, "-", "", false);
        assert!(matches!(result, TransformResult::Value(Value::String(s)) if s == "5551234567"));
    }

    #[test]
    fn literal_replace_no_match() {
        let value = Value::String("hello world".to_string());
        let result = execute_replace(&value, "-", "_", false);
        assert!(matches!(result, TransformResult::Value(Value::String(s)) if s == "hello world"));
    }

    #[test]
    fn regex_replace_whitespace() {
        let value = Value::String("hello    world".to_string());
        let result = execute_replace(&value, r"\s+", " ", true);
        assert!(matches!(result, TransformResult::Value(Value::String(s)) if s == "hello world"));
    }

    #[test]
    fn regex_replace_digits() {
        let value = Value::String("abc123def456".to_string());
        let result = execute_replace(&value, r"\d+", "X", true);
        assert!(matches!(result, TransformResult::Value(Value::String(s)) if s == "abcXdefX"));
    }

    #[test]
    fn regex_replace_with_capture_groups() {
        let value = Value::String("John Smith".to_string());
        let result = execute_replace(&value, r"(\w+) (\w+)", "$2, $1", true);
        assert!(matches!(result, TransformResult::Value(Value::String(s)) if s == "Smith, John"));
    }

    #[test]
    fn regex_invalid_pattern_returns_error() {
        let value = Value::String("test".to_string());
        let result = execute_replace(&value, r"[invalid", "", true);
        assert!(matches!(
            result,
            TransformResult::Error(TransformError::RegexError { .. })
        ));
    }

    #[test]
    fn null_value_returns_null() {
        let result = execute_replace(&Value::Null, "a", "b", false);
        assert!(matches!(result, TransformResult::Value(Value::Null)));
    }

    #[test]
    fn non_string_returns_error() {
        let value = Value::Int(42);
        let result = execute_replace(&value, "4", "X", false);
        assert!(matches!(
            result,
            TransformResult::Error(TransformError::TypeMismatch { .. })
        ));
    }

    #[test]
    fn empty_from_literal() {
        // Replacing empty string inserts between every character
        let value = Value::String("abc".to_string());
        let result = execute_replace(&value, "", "-", false);
        assert!(matches!(result, TransformResult::Value(Value::String(s)) if s == "-a-b-c-"));
    }

    #[test]
    fn empty_to_removes_matches() {
        let value = Value::String("a1b2c3".to_string());
        let result = execute_replace(&value, r"\d", "", true);
        assert!(matches!(result, TransformResult::Value(Value::String(s)) if s == "abc"));
    }
}
