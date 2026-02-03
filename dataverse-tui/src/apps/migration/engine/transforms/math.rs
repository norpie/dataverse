//! Math transform - arithmetic operations.

use dataverse_lib::model::Value;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;

use crate::apps::migration::engine::TransformError;
use crate::apps::migration::engine::TransformResult;
use crate::apps::migration::types::MathOp;

/// Execute the math transform.
///
/// Applies a mathematical operation to `#value`:
/// - `Add(n)` - adds n to value
/// - `Subtract(n)` - subtracts n from value
/// - `Multiply(n)` - multiplies value by n
/// - `Divide(n)` - divides value by n (errors on division by zero)
/// - `Round(places)` - rounds to specified decimal places
///
/// # Type handling
///
/// - Int/Long values are converted to f64 for operations, result is f64
/// - Float values stay as f64
/// - Decimal values use decimal arithmetic for precision
///
/// # Examples
///
/// ```ignore
/// // #value = 10, op = Add(5)
/// // Result: 15.0
///
/// // #value = 100, op = Divide(3)
/// // Result: 33.333...
///
/// // #value = 3.14159, op = Round(2)
/// // Result: 3.14
/// ```
pub fn execute_math(value: &Value, op: &MathOp) -> TransformResult {
    match value {
        Value::Null => TransformResult::Value(Value::Null),
        Value::Decimal(d) => execute_decimal_math(*d, op),
        _ => execute_float_math(value, op),
    }
}

fn execute_float_math(value: &Value, op: &MathOp) -> TransformResult {
    let num = match value_to_f64(value) {
        Some(n) => n,
        None => {
            return TransformResult::Error(TransformError::type_mismatch(
                "number",
                value.type_name(),
            ))
        }
    };

    let result = match op {
        MathOp::Add(n) => num + n,
        MathOp::Subtract(n) => num - n,
        MathOp::Multiply(n) => num * n,
        MathOp::Divide(n) => {
            if *n == 0.0 {
                return TransformResult::Error(TransformError::DivisionByZero);
            }
            num / n
        }
        MathOp::Round(places) => {
            let factor = 10_f64.powi(*places as i32);
            (num * factor).round() / factor
        }
    };

    TransformResult::Value(Value::Float(result))
}

fn execute_decimal_math(value: Decimal, op: &MathOp) -> TransformResult {
    let result = match op {
        MathOp::Add(n) => {
            let n_dec = Decimal::try_from(*n).unwrap_or_else(|_| Decimal::from(*n as i64));
            value + n_dec
        }
        MathOp::Subtract(n) => {
            let n_dec = Decimal::try_from(*n).unwrap_or_else(|_| Decimal::from(*n as i64));
            value - n_dec
        }
        MathOp::Multiply(n) => {
            let n_dec = Decimal::try_from(*n).unwrap_or_else(|_| Decimal::from(*n as i64));
            value * n_dec
        }
        MathOp::Divide(n) => {
            if *n == 0.0 {
                return TransformResult::Error(TransformError::DivisionByZero);
            }
            let n_dec = Decimal::try_from(*n).unwrap_or_else(|_| Decimal::from(*n as i64));
            value / n_dec
        }
        MathOp::Round(places) => value.round_dp(*places),
    };

    TransformResult::Value(Value::Decimal(result))
}

fn value_to_f64(value: &Value) -> Option<f64> {
    match value {
        Value::Int(n) => Some(*n as f64),
        Value::Long(n) => Some(*n as f64),
        Value::Float(n) => Some(*n),
        Value::Decimal(d) => d.to_f64(),
        Value::Money(m) => m.value().to_f64(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==========================================================================
    // Add
    // ==========================================================================

    #[test]
    fn add_to_int() {
        let result = execute_math(&Value::Int(10), &MathOp::Add(5.0));
        if let TransformResult::Value(Value::Float(f)) = result {
            assert!((f - 15.0).abs() < f64::EPSILON);
        } else {
            panic!("Expected float result");
        }
    }

    #[test]
    fn add_to_float() {
        let result = execute_math(&Value::Float(10.5), &MathOp::Add(2.5));
        if let TransformResult::Value(Value::Float(f)) = result {
            assert!((f - 13.0).abs() < f64::EPSILON);
        } else {
            panic!("Expected float result");
        }
    }

    #[test]
    fn add_to_decimal() {
        let result = execute_math(&Value::Decimal(Decimal::from(10)), &MathOp::Add(5.0));
        if let TransformResult::Value(Value::Decimal(d)) = result {
            assert_eq!(d, Decimal::from(15));
        } else {
            panic!("Expected decimal result");
        }
    }

    #[test]
    fn add_negative() {
        let result = execute_math(&Value::Int(10), &MathOp::Add(-3.0));
        if let TransformResult::Value(Value::Float(f)) = result {
            assert!((f - 7.0).abs() < f64::EPSILON);
        } else {
            panic!("Expected float result");
        }
    }

    // ==========================================================================
    // Subtract
    // ==========================================================================

    #[test]
    fn subtract_from_int() {
        let result = execute_math(&Value::Int(10), &MathOp::Subtract(3.0));
        if let TransformResult::Value(Value::Float(f)) = result {
            assert!((f - 7.0).abs() < f64::EPSILON);
        } else {
            panic!("Expected float result");
        }
    }

    #[test]
    fn subtract_resulting_negative() {
        let result = execute_math(&Value::Int(5), &MathOp::Subtract(10.0));
        if let TransformResult::Value(Value::Float(f)) = result {
            assert!((f - (-5.0)).abs() < f64::EPSILON);
        } else {
            panic!("Expected float result");
        }
    }

    // ==========================================================================
    // Multiply
    // ==========================================================================

    #[test]
    fn multiply_int() {
        let result = execute_math(&Value::Int(6), &MathOp::Multiply(7.0));
        if let TransformResult::Value(Value::Float(f)) = result {
            assert!((f - 42.0).abs() < f64::EPSILON);
        } else {
            panic!("Expected float result");
        }
    }

    #[test]
    fn multiply_by_zero() {
        let result = execute_math(&Value::Int(100), &MathOp::Multiply(0.0));
        if let TransformResult::Value(Value::Float(f)) = result {
            assert!((f - 0.0).abs() < f64::EPSILON);
        } else {
            panic!("Expected float result");
        }
    }

    #[test]
    fn multiply_decimal() {
        let result = execute_math(&Value::Decimal(Decimal::from(10)), &MathOp::Multiply(2.5));
        if let TransformResult::Value(Value::Decimal(d)) = result {
            assert_eq!(d, Decimal::from(25));
        } else {
            panic!("Expected decimal result");
        }
    }

    // ==========================================================================
    // Divide
    // ==========================================================================

    #[test]
    fn divide_int() {
        let result = execute_math(&Value::Int(10), &MathOp::Divide(4.0));
        if let TransformResult::Value(Value::Float(f)) = result {
            assert!((f - 2.5).abs() < f64::EPSILON);
        } else {
            panic!("Expected float result");
        }
    }

    #[test]
    fn divide_by_zero_errors() {
        let result = execute_math(&Value::Int(10), &MathOp::Divide(0.0));
        assert!(matches!(
            result,
            TransformResult::Error(TransformError::DivisionByZero)
        ));
    }

    #[test]
    fn divide_decimal_by_zero_errors() {
        let result = execute_math(&Value::Decimal(Decimal::from(10)), &MathOp::Divide(0.0));
        assert!(matches!(
            result,
            TransformResult::Error(TransformError::DivisionByZero)
        ));
    }

    // ==========================================================================
    // Round
    // ==========================================================================

    #[test]
    fn round_to_two_places() {
        let result = execute_math(&Value::Float(3.14159), &MathOp::Round(2));
        if let TransformResult::Value(Value::Float(f)) = result {
            assert!((f - 3.14).abs() < 0.001);
        } else {
            panic!("Expected float result");
        }
    }

    #[test]
    fn round_to_zero_places() {
        let result = execute_math(&Value::Float(3.7), &MathOp::Round(0));
        if let TransformResult::Value(Value::Float(f)) = result {
            assert!((f - 4.0).abs() < f64::EPSILON);
        } else {
            panic!("Expected float result");
        }
    }

    #[test]
    fn round_decimal() {
        let d = Decimal::try_from(3.14159).unwrap();
        let result = execute_math(&Value::Decimal(d), &MathOp::Round(2));
        if let TransformResult::Value(Value::Decimal(d)) = result {
            assert_eq!(d.to_string(), "3.14");
        } else {
            panic!("Expected decimal result");
        }
    }

    #[test]
    fn round_up() {
        let result = execute_math(&Value::Float(2.5), &MathOp::Round(0));
        if let TransformResult::Value(Value::Float(f)) = result {
            assert!((f - 3.0).abs() < f64::EPSILON);
        } else {
            panic!("Expected float result");
        }
    }

    // ==========================================================================
    // Null and error handling
    // ==========================================================================

    #[test]
    fn null_returns_null() {
        let result = execute_math(&Value::Null, &MathOp::Add(5.0));
        assert!(matches!(result, TransformResult::Value(Value::Null)));
    }

    #[test]
    fn non_numeric_returns_error() {
        let result = execute_math(&Value::String("hello".to_string()), &MathOp::Add(5.0));
        assert!(matches!(
            result,
            TransformResult::Error(TransformError::TypeMismatch { .. })
        ));
    }

    #[test]
    fn long_value() {
        let result = execute_math(&Value::Long(1000000000), &MathOp::Multiply(2.0));
        if let TransformResult::Value(Value::Float(f)) = result {
            assert!((f - 2000000000.0).abs() < 1.0);
        } else {
            panic!("Expected float result");
        }
    }
}
