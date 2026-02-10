//! Convert transform - type conversion.

use dataverse_lib::model::Value;
use rust_decimal::Decimal;

use crate::apps::migration::engine::TransformError;
use crate::apps::migration::engine::TransformResult;
use crate::formatting::format_value;

/// Target type for conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConvertTarget {
    Int,
    Decimal,
    String,
    Bool,
}

impl ConvertTarget {
    /// Parse from string representation.
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "int" | "integer" => Some(Self::Int),
            "decimal" | "number" => Some(Self::Decimal),
            "string" | "text" => Some(Self::String),
            "bool" | "boolean" => Some(Self::Bool),
            _ => None,
        }
    }

    /// Get string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Int => "int",
            Self::Decimal => "decimal",
            Self::String => "string",
            Self::Bool => "bool",
        }
    }
}

/// Execute the convert transform.
///
/// Converts `#value` to the target type:
/// - `Int` - converts to 32-bit integer
/// - `Decimal` - converts to arbitrary precision decimal
/// - `String` - converts using display formatting
/// - `Bool` - converts to boolean
///
/// # Conversion Rules
///
/// To Int:
/// - Int/Long → truncate to i32
/// - Float/Decimal → truncate to i32
/// - String → parse as integer
/// - Bool → 1 or 0
///
/// To Decimal:
/// - Int/Long/Float → direct conversion
/// - Decimal → unchanged
/// - String → parse as decimal
/// - Bool → 1 or 0
///
/// To String:
/// - Any → uses format_value().display
///
/// To Bool:
/// - Int/Long → non-zero is true
/// - Float/Decimal → non-zero is true
/// - String → "true"/"1"/"yes" is true, "false"/"0"/"no" is false
/// - Bool → unchanged
pub fn execute_convert(value: &Value, target: ConvertTarget) -> TransformResult {
    if matches!(value, Value::Null) {
        return TransformResult::Value(Value::Null);
    }

    match target {
        ConvertTarget::Int => convert_to_int(value),
        ConvertTarget::Decimal => convert_to_decimal(value),
        ConvertTarget::String => convert_to_string(value),
        ConvertTarget::Bool => convert_to_bool(value),
    }
}

fn convert_to_int(value: &Value) -> TransformResult {
    let result = match value {
        Value::Int(n) => *n,
        Value::Long(n) => *n as i32,
        Value::Float(n) => *n as i32,
        Value::Decimal(d) => d.to_string().parse::<f64>().map(|f| f as i32).unwrap_or(0),
        Value::Bool(b) => {
            if *b {
                1
            } else {
                0
            }
        }
        Value::String(s) => {
            match s.trim().parse::<i32>() {
                Ok(n) => n,
                Err(_) => {
                    // Try parsing as float and truncating
                    match s.trim().parse::<f64>() {
                        Ok(f) => f as i32,
                        Err(_) => {
                            return TransformResult::Error(TransformError::type_mismatch(
                                "int",
                                format!("string '{}'", s),
                            ));
                        }
                    }
                }
            }
        }
        Value::OptionSet(o) => o.value,
        other => {
            return TransformResult::Error(TransformError::type_mismatch("int", other.type_name()));
        }
    };
    TransformResult::Value(Value::Int(result))
}

fn convert_to_decimal(value: &Value) -> TransformResult {
    let result = match value {
        Value::Int(n) => Decimal::from(*n),
        Value::Long(n) => Decimal::from(*n),
        Value::Float(n) => Decimal::try_from(*n).unwrap_or_else(|_| Decimal::from(*n as i64)),
        Value::Decimal(d) => *d,
        Value::Bool(b) => {
            if *b {
                Decimal::from(1)
            } else {
                Decimal::from(0)
            }
        }
        Value::String(s) => match s.trim().parse::<Decimal>() {
            Ok(d) => d,
            Err(_) => {
                return TransformResult::Error(TransformError::type_mismatch(
                    "decimal",
                    format!("string '{}'", s),
                ));
            }
        },
        Value::Money(m) => m.value(),
        other => {
            return TransformResult::Error(TransformError::type_mismatch(
                "decimal",
                other.type_name(),
            ));
        }
    };
    TransformResult::Value(Value::Decimal(result))
}

fn convert_to_string(value: &Value) -> TransformResult {
    let formatted = format_value(value);
    TransformResult::Value(Value::String(formatted.display))
}

fn convert_to_bool(value: &Value) -> TransformResult {
    let result = match value {
        Value::Bool(b) => *b,
        Value::Int(n) => *n != 0,
        Value::Long(n) => *n != 0,
        Value::Float(n) => *n != 0.0,
        Value::Decimal(d) => !d.is_zero(),
        Value::String(s) => match s.trim().to_lowercase().as_str() {
            "true" | "1" | "yes" | "y" => true,
            "false" | "0" | "no" | "n" | "" => false,
            _ => {
                return TransformResult::Error(TransformError::type_mismatch(
                    "bool",
                    format!("string '{}'", s),
                ));
            }
        },
        other => {
            return TransformResult::Error(TransformError::type_mismatch(
                "bool",
                other.type_name(),
            ));
        }
    };
    TransformResult::Value(Value::Bool(result))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==========================================================================
    // Convert to Int
    // ==========================================================================

    #[test]
    fn int_to_int() {
        let result = execute_convert(&Value::Int(42), ConvertTarget::Int);
        assert!(matches!(result, TransformResult::Value(Value::Int(42))));
    }

    #[test]
    fn long_to_int() {
        let result = execute_convert(&Value::Long(1000), ConvertTarget::Int);
        assert!(matches!(result, TransformResult::Value(Value::Int(1000))));
    }

    #[test]
    fn float_to_int_truncates() {
        let result = execute_convert(&Value::Float(3.9), ConvertTarget::Int);
        assert!(matches!(result, TransformResult::Value(Value::Int(3))));
    }

    #[test]
    fn string_to_int() {
        let result = execute_convert(&Value::String("123".to_string()), ConvertTarget::Int);
        assert!(matches!(result, TransformResult::Value(Value::Int(123))));
    }

    #[test]
    fn string_float_to_int_truncates() {
        let result = execute_convert(&Value::String("45.7".to_string()), ConvertTarget::Int);
        assert!(matches!(result, TransformResult::Value(Value::Int(45))));
    }

    #[test]
    fn bool_to_int() {
        let result = execute_convert(&Value::Bool(true), ConvertTarget::Int);
        assert!(matches!(result, TransformResult::Value(Value::Int(1))));

        let result = execute_convert(&Value::Bool(false), ConvertTarget::Int);
        assert!(matches!(result, TransformResult::Value(Value::Int(0))));
    }

    #[test]
    fn invalid_string_to_int_errors() {
        let result = execute_convert(&Value::String("abc".to_string()), ConvertTarget::Int);
        assert!(matches!(
            result,
            TransformResult::Error(TransformError::TypeMismatch { .. })
        ));
    }

    // ==========================================================================
    // Convert to Decimal
    // ==========================================================================

    #[test]
    fn int_to_decimal() {
        let result = execute_convert(&Value::Int(42), ConvertTarget::Decimal);
        assert!(
            matches!(result, TransformResult::Value(Value::Decimal(d)) if d == Decimal::from(42))
        );
    }

    #[test]
    fn string_to_decimal() {
        let result = execute_convert(&Value::String("123.45".to_string()), ConvertTarget::Decimal);
        assert!(
            matches!(result, TransformResult::Value(Value::Decimal(d)) if d == Decimal::try_from(123.45).unwrap())
        );
    }

    #[test]
    fn invalid_string_to_decimal_errors() {
        let result = execute_convert(
            &Value::String("not a number".to_string()),
            ConvertTarget::Decimal,
        );
        assert!(matches!(
            result,
            TransformResult::Error(TransformError::TypeMismatch { .. })
        ));
    }

    // ==========================================================================
    // Convert to String
    // ==========================================================================

    #[test]
    fn int_to_string() {
        let result = execute_convert(&Value::Int(42), ConvertTarget::String);
        assert!(matches!(result, TransformResult::Value(Value::String(s)) if s == "42"));
    }

    #[test]
    fn bool_to_string() {
        let result = execute_convert(&Value::Bool(true), ConvertTarget::String);
        assert!(matches!(result, TransformResult::Value(Value::String(s)) if s == "Yes"));

        let result = execute_convert(&Value::Bool(false), ConvertTarget::String);
        assert!(matches!(result, TransformResult::Value(Value::String(s)) if s == "No"));
    }

    #[test]
    fn string_to_string() {
        let result = execute_convert(&Value::String("hello".to_string()), ConvertTarget::String);
        assert!(matches!(result, TransformResult::Value(Value::String(s)) if s == "hello"));
    }

    // ==========================================================================
    // Convert to Bool
    // ==========================================================================

    #[test]
    fn bool_to_bool() {
        let result = execute_convert(&Value::Bool(true), ConvertTarget::Bool);
        assert!(matches!(result, TransformResult::Value(Value::Bool(true))));
    }

    #[test]
    fn int_to_bool() {
        let result = execute_convert(&Value::Int(1), ConvertTarget::Bool);
        assert!(matches!(result, TransformResult::Value(Value::Bool(true))));

        let result = execute_convert(&Value::Int(0), ConvertTarget::Bool);
        assert!(matches!(result, TransformResult::Value(Value::Bool(false))));

        let result = execute_convert(&Value::Int(-5), ConvertTarget::Bool);
        assert!(matches!(result, TransformResult::Value(Value::Bool(true))));
    }

    #[test]
    fn string_to_bool() {
        for s in ["true", "TRUE", "True", "1", "yes", "YES", "y", "Y"] {
            let result = execute_convert(&Value::String(s.to_string()), ConvertTarget::Bool);
            assert!(
                matches!(result, TransformResult::Value(Value::Bool(true))),
                "Failed for '{}'",
                s
            );
        }

        for s in ["false", "FALSE", "False", "0", "no", "NO", "n", "N", ""] {
            let result = execute_convert(&Value::String(s.to_string()), ConvertTarget::Bool);
            assert!(
                matches!(result, TransformResult::Value(Value::Bool(false))),
                "Failed for '{}'",
                s
            );
        }
    }

    #[test]
    fn invalid_string_to_bool_errors() {
        let result = execute_convert(&Value::String("maybe".to_string()), ConvertTarget::Bool);
        assert!(matches!(
            result,
            TransformResult::Error(TransformError::TypeMismatch { .. })
        ));
    }

    // ==========================================================================
    // Null handling
    // ==========================================================================

    #[test]
    fn null_converts_to_null() {
        let result = execute_convert(&Value::Null, ConvertTarget::Int);
        assert!(matches!(result, TransformResult::Value(Value::Null)));

        let result = execute_convert(&Value::Null, ConvertTarget::String);
        assert!(matches!(result, TransformResult::Value(Value::Null)));

        let result = execute_convert(&Value::Null, ConvertTarget::Bool);
        assert!(matches!(result, TransformResult::Value(Value::Null)));
    }

    // ==========================================================================
    // ConvertTarget parsing
    // ==========================================================================

    #[test]
    fn convert_target_from_str() {
        assert_eq!(ConvertTarget::from_str("int"), Some(ConvertTarget::Int));
        assert_eq!(ConvertTarget::from_str("INTEGER"), Some(ConvertTarget::Int));
        assert_eq!(
            ConvertTarget::from_str("decimal"),
            Some(ConvertTarget::Decimal)
        );
        assert_eq!(
            ConvertTarget::from_str("number"),
            Some(ConvertTarget::Decimal)
        );
        assert_eq!(
            ConvertTarget::from_str("string"),
            Some(ConvertTarget::String)
        );
        assert_eq!(ConvertTarget::from_str("text"), Some(ConvertTarget::String));
        assert_eq!(ConvertTarget::from_str("bool"), Some(ConvertTarget::Bool));
        assert_eq!(
            ConvertTarget::from_str("boolean"),
            Some(ConvertTarget::Bool)
        );
        assert_eq!(ConvertTarget::from_str("unknown"), None);
    }
}
