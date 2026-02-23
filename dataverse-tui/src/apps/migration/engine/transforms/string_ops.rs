//! String operations transform - chain of string manipulations.

use dataverse_lib::model::Value;

use crate::apps::migration::engine::TransformError;
use crate::apps::migration::engine::TransformResult;
use crate::apps::migration::types::StringOp;

/// Execute the string_ops transform.
///
/// Applies a sequence of string operations to `#value`:
/// - `Uppercase` - convert to uppercase
/// - `Lowercase` - convert to lowercase
/// - `Trim` - remove leading and trailing whitespace
/// - `TrimStart` - remove leading whitespace
/// - `TrimEnd` - remove trailing whitespace
///
/// Operations are applied in order.
///
/// # Examples
///
/// ```ignore
/// // #value = "  Hello World  "
/// // ops = [Trim, Lowercase]
/// // Result: "hello world"
/// ```
///
/// # Errors
///
/// - `TypeMismatch` if `#value` is not a string
pub fn execute_string_ops(value: &Value, ops: &[StringOp]) -> TransformResult {
    let input = match value {
        Value::String(s) => s.clone(),
        Value::Guid(g) => g.to_string(),
        Value::Null => return TransformResult::Value(Value::Null),
        other => {
            return TransformResult::Error(TransformError::type_mismatch(
                "string",
                other.type_name(),
            ));
        }
    };

    let result = ops.iter().fold(input, |acc, op| apply_op(&acc, op));
    TransformResult::Value(Value::String(result))
}

/// Apply a single string operation.
fn apply_op(s: &str, op: &StringOp) -> String {
    match op {
        StringOp::Uppercase => s.to_uppercase(),
        StringOp::Lowercase => s.to_lowercase(),
        StringOp::Trim => s.trim().to_string(),
        StringOp::TrimStart => s.trim_start().to_string(),
        StringOp::TrimEnd => s.trim_end().to_string(),
        StringOp::Truncate(max_len) => {
            if s.len() <= *max_len {
                s.to_string()
            } else {
                s.chars().take(*max_len).collect()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uppercase() {
        let value = Value::String("hello world".to_string());
        let result = execute_string_ops(&value, &[StringOp::Uppercase]);
        assert!(matches!(result, TransformResult::Value(Value::String(s)) if s == "HELLO WORLD"));
    }

    #[test]
    fn lowercase() {
        let value = Value::String("Hello World".to_string());
        let result = execute_string_ops(&value, &[StringOp::Lowercase]);
        assert!(matches!(result, TransformResult::Value(Value::String(s)) if s == "hello world"));
    }

    #[test]
    fn trim() {
        let value = Value::String("  hello world  ".to_string());
        let result = execute_string_ops(&value, &[StringOp::Trim]);
        assert!(matches!(result, TransformResult::Value(Value::String(s)) if s == "hello world"));
    }

    #[test]
    fn trim_start() {
        let value = Value::String("  hello world  ".to_string());
        let result = execute_string_ops(&value, &[StringOp::TrimStart]);
        assert!(matches!(result, TransformResult::Value(Value::String(s)) if s == "hello world  "));
    }

    #[test]
    fn trim_end() {
        let value = Value::String("  hello world  ".to_string());
        let result = execute_string_ops(&value, &[StringOp::TrimEnd]);
        assert!(matches!(result, TransformResult::Value(Value::String(s)) if s == "  hello world"));
    }

    #[test]
    fn chained_operations() {
        let value = Value::String("  Hello World  ".to_string());
        let result = execute_string_ops(&value, &[StringOp::Trim, StringOp::Lowercase]);
        assert!(matches!(result, TransformResult::Value(Value::String(s)) if s == "hello world"));
    }

    #[test]
    fn multiple_operations_order_matters() {
        let value = Value::String("  HELLO  ".to_string());

        // Trim then lowercase
        let result1 = execute_string_ops(&value, &[StringOp::Trim, StringOp::Lowercase]);
        assert!(matches!(result1, TransformResult::Value(Value::String(s)) if s == "hello"));

        // Lowercase then trim (same result in this case)
        let result2 = execute_string_ops(&value, &[StringOp::Lowercase, StringOp::Trim]);
        assert!(matches!(result2, TransformResult::Value(Value::String(s)) if s == "hello"));
    }

    #[test]
    fn empty_ops_returns_unchanged() {
        let value = Value::String("hello".to_string());
        let result = execute_string_ops(&value, &[]);
        assert!(matches!(result, TransformResult::Value(Value::String(s)) if s == "hello"));
    }

    #[test]
    fn null_value_returns_null() {
        let result = execute_string_ops(&Value::Null, &[StringOp::Trim]);
        assert!(matches!(result, TransformResult::Value(Value::Null)));
    }

    #[test]
    fn non_string_returns_error() {
        let value = Value::Int(42);
        let result = execute_string_ops(&value, &[StringOp::Uppercase]);
        assert!(matches!(
            result,
            TransformResult::Error(TransformError::TypeMismatch { .. })
        ));
    }

    #[test]
    fn unicode_operations() {
        let value = Value::String("Héllo Wörld".to_string());

        let upper = execute_string_ops(&value, &[StringOp::Uppercase]);
        assert!(matches!(upper, TransformResult::Value(Value::String(s)) if s == "HÉLLO WÖRLD"));

        let lower = execute_string_ops(&value, &[StringOp::Lowercase]);
        assert!(matches!(lower, TransformResult::Value(Value::String(s)) if s == "héllo wörld"));
    }
}
