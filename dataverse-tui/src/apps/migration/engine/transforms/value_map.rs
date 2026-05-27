//! Value map transform - lookup table mapping.

use dataverse_lib::model::Value;
use dataverse_lib::model::types::MultiSelectOptionSetValue;

use crate::apps::migration::engine::TransformError;
use crate::apps::migration::engine::TransformResult;

/// A single mapping entry.
#[derive(Debug, Clone, PartialEq)]
pub struct ValueMapping {
    pub from: Value,
    pub to: Value,
}

impl ValueMapping {
    pub fn new(from: impl Into<Value>, to: impl Into<Value>) -> Self {
        Self {
            from: from.into(),
            to: to.into(),
        }
    }
}

/// Execute the value_map transform.
///
/// Maps `#value` through a lookup table. If the value matches a `from` entry,
/// returns the corresponding `to` value.
///
/// # Examples
///
/// ```ignore
/// // Mapping: [(1, "Active"), (2, "Inactive")]
/// // #value = 1
/// // Result: "Active"
///
/// // Mapping: [("US", "United States"), ("UK", "United Kingdom")]
/// // #value = "UK"
/// // Result: "United Kingdom"
/// ```
///
/// # Multi-select handling
///
/// For multi-select option sets, each value is mapped individually and
/// the results are combined back into a multi-select.
///
/// # Errors
///
/// - Returns `Value::Null` if no mapping exists for the value
pub fn execute_value_map(value: &Value, mappings: &[ValueMapping]) -> TransformResult {
    match value {
        Value::Null => TransformResult::Value(Value::Null),
        Value::MultiOptionSet(mos) => map_multi_select(mos, mappings),
        _ => map_single_value(value, mappings),
    }
}

fn map_single_value(value: &Value, mappings: &[ValueMapping]) -> TransformResult {
    for mapping in mappings {
        if values_equal(value, &mapping.from) {
            return TransformResult::Value(mapping.to.clone());
        }
    }

    TransformResult::Value(Value::Null)
}

fn map_multi_select(mos: &MultiSelectOptionSetValue, mappings: &[ValueMapping]) -> TransformResult {
    let mut mapped_values = Vec::new();
    let mut mapped_labels = Vec::new();

    for &val in &mos.values {
        let from_value = Value::Int(val);
        let mut found = false;

        for mapping in mappings {
            if values_equal(&from_value, &mapping.from) {
                // Extract the mapped value
                match &mapping.to {
                    Value::Int(n) => {
                        mapped_values.push(*n);
                        mapped_labels.push(n.to_string());
                    }
                    Value::String(s) => {
                        // If mapping to string, use original value but track the label
                        mapped_values.push(val);
                        mapped_labels.push(s.clone());
                    }
                    _ => {
                        mapped_values.push(val);
                    }
                }
                found = true;
                break;
            }
        }

        if !found {
            // Skip unmapped values in multi-select
            continue;
        }
    }

    TransformResult::Value(Value::MultiOptionSet(MultiSelectOptionSetValue {
        values: mapped_values,
        labels: if mapped_labels.is_empty() {
            None
        } else {
            Some(mapped_labels)
        },
    }))
}

use crate::apps::migration::engine::util::values_equal;

#[cfg(test)]
mod tests {
    use super::*;
    use dataverse_lib::model::types::OptionSetValue;

    fn status_mappings() -> Vec<ValueMapping> {
        vec![
            ValueMapping::new(1, "Active"),
            ValueMapping::new(2, "Inactive"),
            ValueMapping::new(3, "Pending"),
        ]
    }

    fn country_mappings() -> Vec<ValueMapping> {
        vec![
            ValueMapping::new("US", "United States"),
            ValueMapping::new("UK", "United Kingdom"),
            ValueMapping::new("DE", "Germany"),
        ]
    }

    #[test]
    fn map_int_to_string() {
        let mappings = status_mappings();
        let result = execute_value_map(&Value::Int(1), &mappings);
        assert!(matches!(result, TransformResult::Value(Value::String(s)) if s == "Active"));
    }

    #[test]
    fn map_int_second_entry() {
        let mappings = status_mappings();
        let result = execute_value_map(&Value::Int(2), &mappings);
        assert!(matches!(result, TransformResult::Value(Value::String(s)) if s == "Inactive"));
    }

    #[test]
    fn map_string_to_string() {
        let mappings = country_mappings();
        let result = execute_value_map(&Value::String("UK".to_string()), &mappings);
        assert!(
            matches!(result, TransformResult::Value(Value::String(s)) if s == "United Kingdom")
        );
    }

    #[test]
    fn map_not_found_returns_null() {
        let mappings = status_mappings();
        let result = execute_value_map(&Value::Int(99), &mappings);
        assert!(matches!(result, TransformResult::Value(Value::Null)));
    }

    #[test]
    fn map_null_returns_null() {
        let mappings = status_mappings();
        let result = execute_value_map(&Value::Null, &mappings);
        assert!(matches!(result, TransformResult::Value(Value::Null)));
    }

    #[test]
    fn map_option_set_value() {
        let mappings = status_mappings();
        let os = Value::OptionSet(OptionSetValue {
            value: 1,
            label: Some("Old Label".to_string()),
        });
        let result = execute_value_map(&os, &mappings);
        assert!(matches!(result, TransformResult::Value(Value::String(s)) if s == "Active"));
    }

    #[test]
    fn map_int_to_int() {
        let mappings = vec![ValueMapping::new(1, 100), ValueMapping::new(2, 200)];
        let result = execute_value_map(&Value::Int(1), &mappings);
        assert!(matches!(result, TransformResult::Value(Value::Int(100))));
    }

    #[test]
    fn map_string_to_int() {
        let mappings = vec![
            ValueMapping::new("low", 1),
            ValueMapping::new("medium", 2),
            ValueMapping::new("high", 3),
        ];
        let result = execute_value_map(&Value::String("medium".to_string()), &mappings);
        assert!(matches!(result, TransformResult::Value(Value::Int(2))));
    }

    #[test]
    fn map_empty_mappings_returns_null() {
        let result = execute_value_map(&Value::Int(1), &[]);
        assert!(matches!(result, TransformResult::Value(Value::Null)));
    }

    #[test]
    fn map_bool_values() {
        let mappings = vec![
            ValueMapping::new(Value::Bool(true), "Yes"),
            ValueMapping::new(Value::Bool(false), "No"),
        ];
        let result = execute_value_map(&Value::Bool(true), &mappings);
        assert!(matches!(result, TransformResult::Value(Value::String(s)) if s == "Yes"));
    }

    #[test]
    fn map_first_match_wins() {
        let mappings = vec![
            ValueMapping::new(1, "First"),
            ValueMapping::new(1, "Second"),
        ];
        let result = execute_value_map(&Value::Int(1), &mappings);
        assert!(matches!(result, TransformResult::Value(Value::String(s)) if s == "First"));
    }
}
