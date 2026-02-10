//! Shared utility functions for the transform engine.

use dataverse_lib::model::Value;

/// Check equality between two values, with cross-type flexibility.
///
/// Handles numeric coercion (Int↔Long, OptionSet↔Int) and
/// approximate float comparison.
pub fn values_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Null, Value::Null) => true,
        (Value::Bool(a), Value::Bool(b)) => a == b,
        (Value::Int(a), Value::Int(b)) => a == b,
        (Value::Long(a), Value::Long(b)) => a == b,
        (Value::Int(a), Value::Long(b)) => (*a as i64) == *b,
        (Value::Long(a), Value::Int(b)) => *a == (*b as i64),
        (Value::Float(a), Value::Float(b)) => (a - b).abs() < f64::EPSILON,
        (Value::Decimal(a), Value::Decimal(b)) => a == b,
        (Value::String(a), Value::String(b)) => a == b,
        (Value::Guid(a), Value::Guid(b)) => a == b,
        (Value::OptionSet(a), Value::OptionSet(b)) => a.value == b.value,
        (Value::OptionSet(a), Value::Int(b)) => a.value == *b,
        (Value::Int(a), Value::OptionSet(b)) => *a == b.value,
        _ => false,
    }
}
