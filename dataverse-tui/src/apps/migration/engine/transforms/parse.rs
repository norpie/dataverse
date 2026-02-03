//! Parse transforms - string parsing to typed values.

use chrono::DateTime;
use chrono::NaiveDateTime;
use chrono::Utc;
use dataverse_lib::model::Value;
use rust_decimal::Decimal;

use crate::apps::migration::engine::TransformError;
use crate::apps::migration::engine::TransformResult;

/// Execute the parse_int transform.
///
/// Parses `#value` string to an integer.
///
/// # Examples
///
/// ```ignore
/// // #value = "123"
/// // Result: 123 (Int)
///
/// // #value = "  -456  "
/// // Result: -456 (Int)
/// ```
pub fn execute_parse_int(value: &Value) -> TransformResult {
    match value {
        Value::String(s) => match s.trim().parse::<i32>() {
            Ok(n) => TransformResult::Value(Value::Int(n)),
            Err(e) => TransformResult::Error(TransformError::ParseError {
                message: format!("Cannot parse '{}' as integer: {}", s, e),
            }),
        },
        Value::Null => TransformResult::Value(Value::Null),
        Value::Int(n) => TransformResult::Value(Value::Int(*n)),
        Value::Long(n) => TransformResult::Value(Value::Int(*n as i32)),
        other => TransformResult::Error(TransformError::type_mismatch("string", other.type_name())),
    }
}

/// Execute the parse_decimal transform.
///
/// Parses `#value` string to a decimal.
///
/// # Examples
///
/// ```ignore
/// // #value = "123.45"
/// // Result: 123.45 (Decimal)
///
/// // #value = "  -0.001  "
/// // Result: -0.001 (Decimal)
/// ```
pub fn execute_parse_decimal(value: &Value) -> TransformResult {
    match value {
        Value::String(s) => match s.trim().parse::<Decimal>() {
            Ok(d) => TransformResult::Value(Value::Decimal(d)),
            Err(e) => TransformResult::Error(TransformError::ParseError {
                message: format!("Cannot parse '{}' as decimal: {}", s, e),
            }),
        },
        Value::Null => TransformResult::Value(Value::Null),
        Value::Int(n) => TransformResult::Value(Value::Decimal(Decimal::from(*n))),
        Value::Long(n) => TransformResult::Value(Value::Decimal(Decimal::from(*n))),
        Value::Decimal(d) => TransformResult::Value(Value::Decimal(*d)),
        other => TransformResult::Error(TransformError::type_mismatch("string", other.type_name())),
    }
}

/// Execute the parse_date transform.
///
/// Parses `#value` string to a datetime using a strftime format string.
///
/// # Examples
///
/// ```ignore
/// // #value = "2024-01-15", format = "%Y-%m-%d"
/// // Result: 2024-01-15T00:00:00Z (DateTime)
///
/// // #value = "15/01/2024 14:30", format = "%d/%m/%Y %H:%M"
/// // Result: 2024-01-15T14:30:00Z (DateTime)
/// ```
///
/// # Format Specifiers
///
/// Common strftime specifiers:
/// - `%Y` - 4-digit year (2024)
/// - `%m` - 2-digit month (01-12)
/// - `%d` - 2-digit day (01-31)
/// - `%H` - 24-hour hour (00-23)
/// - `%M` - minute (00-59)
/// - `%S` - second (00-59)
/// - `%y` - 2-digit year (24)
/// - `%b` - abbreviated month name (Jan)
/// - `%B` - full month name (January)
pub fn execute_parse_date(value: &Value, format: &str) -> TransformResult {
    match value {
        Value::String(s) => {
            match NaiveDateTime::parse_from_str(s.trim(), format) {
                Ok(naive) => {
                    let dt = DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc);
                    TransformResult::Value(Value::DateTime(dt))
                }
                Err(_) => {
                    // Try parsing as date only (no time component)
                    match chrono::NaiveDate::parse_from_str(s.trim(), format) {
                        Ok(date) => {
                            let naive = date.and_hms_opt(0, 0, 0).unwrap();
                            let dt = DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc);
                            TransformResult::Value(Value::DateTime(dt))
                        }
                        Err(e) => TransformResult::Error(TransformError::DateFormatError {
                            format: format.to_string(),
                            message: format!("Cannot parse '{}': {}", s, e),
                        }),
                    }
                }
            }
        }
        Value::Null => TransformResult::Value(Value::Null),
        Value::DateTime(dt) => TransformResult::Value(Value::DateTime(*dt)),
        other => TransformResult::Error(TransformError::type_mismatch("string", other.type_name())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==========================================================================
    // parse_int
    // ==========================================================================

    #[test]
    fn parse_int_positive() {
        let result = execute_parse_int(&Value::String("123".to_string()));
        assert!(matches!(result, TransformResult::Value(Value::Int(123))));
    }

    #[test]
    fn parse_int_negative() {
        let result = execute_parse_int(&Value::String("-456".to_string()));
        assert!(matches!(result, TransformResult::Value(Value::Int(-456))));
    }

    #[test]
    fn parse_int_with_whitespace() {
        let result = execute_parse_int(&Value::String("  789  ".to_string()));
        assert!(matches!(result, TransformResult::Value(Value::Int(789))));
    }

    #[test]
    fn parse_int_invalid() {
        let result = execute_parse_int(&Value::String("abc".to_string()));
        assert!(matches!(
            result,
            TransformResult::Error(TransformError::ParseError { .. })
        ));
    }

    #[test]
    fn parse_int_float_string_fails() {
        let result = execute_parse_int(&Value::String("12.34".to_string()));
        assert!(matches!(
            result,
            TransformResult::Error(TransformError::ParseError { .. })
        ));
    }

    #[test]
    fn parse_int_null() {
        let result = execute_parse_int(&Value::Null);
        assert!(matches!(result, TransformResult::Value(Value::Null)));
    }

    #[test]
    fn parse_int_already_int() {
        let result = execute_parse_int(&Value::Int(42));
        assert!(matches!(result, TransformResult::Value(Value::Int(42))));
    }

    // ==========================================================================
    // parse_decimal
    // ==========================================================================

    #[test]
    fn parse_decimal_integer() {
        let result = execute_parse_decimal(&Value::String("123".to_string()));
        assert!(
            matches!(result, TransformResult::Value(Value::Decimal(d)) if d == Decimal::from(123))
        );
    }

    #[test]
    fn parse_decimal_fractional() {
        let result = execute_parse_decimal(&Value::String("123.456".to_string()));
        if let TransformResult::Value(Value::Decimal(d)) = result {
            assert_eq!(d.to_string(), "123.456");
        } else {
            panic!("Expected decimal value");
        }
    }

    #[test]
    fn parse_decimal_negative() {
        let result = execute_parse_decimal(&Value::String("-0.001".to_string()));
        if let TransformResult::Value(Value::Decimal(d)) = result {
            assert_eq!(d.to_string(), "-0.001");
        } else {
            panic!("Expected decimal value");
        }
    }

    #[test]
    fn parse_decimal_with_whitespace() {
        let result = execute_parse_decimal(&Value::String("  45.67  ".to_string()));
        if let TransformResult::Value(Value::Decimal(d)) = result {
            assert_eq!(d.to_string(), "45.67");
        } else {
            panic!("Expected decimal value");
        }
    }

    #[test]
    fn parse_decimal_invalid() {
        let result = execute_parse_decimal(&Value::String("not a number".to_string()));
        assert!(matches!(
            result,
            TransformResult::Error(TransformError::ParseError { .. })
        ));
    }

    #[test]
    fn parse_decimal_null() {
        let result = execute_parse_decimal(&Value::Null);
        assert!(matches!(result, TransformResult::Value(Value::Null)));
    }

    // ==========================================================================
    // parse_date
    // ==========================================================================

    #[test]
    fn parse_date_iso() {
        let result = execute_parse_date(&Value::String("2024-01-15".to_string()), "%Y-%m-%d");
        if let TransformResult::Value(Value::DateTime(dt)) = result {
            assert_eq!(dt.format("%Y-%m-%d").to_string(), "2024-01-15");
        } else {
            panic!("Expected datetime value, got {:?}", result);
        }
    }

    #[test]
    fn parse_date_with_time() {
        let result = execute_parse_date(
            &Value::String("2024-01-15 14:30:00".to_string()),
            "%Y-%m-%d %H:%M:%S",
        );
        if let TransformResult::Value(Value::DateTime(dt)) = result {
            assert_eq!(
                dt.format("%Y-%m-%d %H:%M:%S").to_string(),
                "2024-01-15 14:30:00"
            );
        } else {
            panic!("Expected datetime value, got {:?}", result);
        }
    }

    #[test]
    fn parse_date_european_format() {
        let result = execute_parse_date(&Value::String("15/01/2024".to_string()), "%d/%m/%Y");
        if let TransformResult::Value(Value::DateTime(dt)) = result {
            assert_eq!(dt.format("%Y-%m-%d").to_string(), "2024-01-15");
        } else {
            panic!("Expected datetime value, got {:?}", result);
        }
    }

    #[test]
    fn parse_date_us_format() {
        let result = execute_parse_date(&Value::String("01/15/2024".to_string()), "%m/%d/%Y");
        if let TransformResult::Value(Value::DateTime(dt)) = result {
            assert_eq!(dt.format("%Y-%m-%d").to_string(), "2024-01-15");
        } else {
            panic!("Expected datetime value, got {:?}", result);
        }
    }

    #[test]
    fn parse_date_invalid_format() {
        let result = execute_parse_date(&Value::String("2024-01-15".to_string()), "%d/%m/%Y");
        assert!(matches!(
            result,
            TransformResult::Error(TransformError::DateFormatError { .. })
        ));
    }

    #[test]
    fn parse_date_invalid_date() {
        let result = execute_parse_date(&Value::String("not a date".to_string()), "%Y-%m-%d");
        assert!(matches!(
            result,
            TransformResult::Error(TransformError::DateFormatError { .. })
        ));
    }

    #[test]
    fn parse_date_null() {
        let result = execute_parse_date(&Value::Null, "%Y-%m-%d");
        assert!(matches!(result, TransformResult::Value(Value::Null)));
    }

    #[test]
    fn parse_date_already_datetime() {
        let dt = Utc::now();
        let result = execute_parse_date(&Value::DateTime(dt), "%Y-%m-%d");
        assert!(matches!(result, TransformResult::Value(Value::DateTime(_))));
    }

    #[test]
    fn parse_date_with_whitespace() {
        let result = execute_parse_date(&Value::String("  2024-01-15  ".to_string()), "%Y-%m-%d");
        if let TransformResult::Value(Value::DateTime(dt)) = result {
            assert_eq!(dt.format("%Y-%m-%d").to_string(), "2024-01-15");
        } else {
            panic!("Expected datetime value, got {:?}", result);
        }
    }
}
