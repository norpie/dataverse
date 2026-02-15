//! Field diffing — compare transformed output against a matched target record.
//!
//! Produces a list of field-level differences. Only fields that actually changed
//! are included in the output.

use std::collections::HashMap;

use dataverse_lib::model::Record;
use dataverse_lib::model::Value;

use crate::apps::migration::engine::util::values_equal;

// =============================================================================
// Types
// =============================================================================

/// A single field-level difference between transformed output and target record.
#[derive(Debug, Clone)]
pub struct FieldDiff {
    /// Target field name.
    pub field: String,
    /// The new (transformed) value.
    pub new_value: Value,
    /// The old (target) value. `Value::Null` if the field was missing from the target.
    pub old_value: Value,
}

// =============================================================================
// Diffing Logic
// =============================================================================

/// Compare transformed field values against a target record.
///
/// For each field in `transformed`:
/// - If target has no value and transformed is `Null` → no diff
/// - If target has no value and transformed is not `Null` → diff (new field)
/// - If `values_equal` → no diff
/// - Otherwise → diff with both values
pub fn diff_fields(transformed: &HashMap<String, Value>, target: &Record) -> Vec<FieldDiff> {
    let mut diffs = Vec::new();

    for (field, new_value) in transformed {
        match target.get(field) {
            Some(old_value) => {
                if !values_equal(new_value, old_value) {
                    log::trace!(
                        "diff_fields: field={:?} new={:?} old={:?}",
                        field,
                        std::mem::discriminant(new_value),
                        std::mem::discriminant(old_value)
                    );
                    log::trace!(
                        "diff_fields: field={:?} new_val={:?} old_val={:?}",
                        field,
                        new_value,
                        old_value
                    );
                    diffs.push(FieldDiff {
                        field: field.clone(),
                        new_value: new_value.clone(),
                        old_value: old_value.clone(),
                    });
                }
            }
            None => {
                // Target doesn't have this field
                if !matches!(new_value, Value::Null) {
                    diffs.push(FieldDiff {
                        field: field.clone(),
                        new_value: new_value.clone(),
                        old_value: Value::Null,
                    });
                }
            }
        }
    }

    diffs
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use dataverse_lib::model::types::OptionSetValue;
    use dataverse_lib::model::Entity;

    fn make_target(fields: Vec<(&str, Value)>) -> Record {
        let mut record = Record::new(Entity::logical("account"));
        for (field, value) in fields {
            record = record.set(field, value);
        }
        record
    }

    fn make_transformed(fields: Vec<(&str, Value)>) -> HashMap<String, Value> {
        fields
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect()
    }

    #[test]
    fn no_diffs_when_identical() {
        let transformed = make_transformed(vec![
            ("name", Value::from("Acme")),
            ("revenue", Value::Int(1000)),
        ]);
        let target = make_target(vec![
            ("name", Value::from("Acme")),
            ("revenue", Value::Int(1000)),
        ]);

        let diffs = diff_fields(&transformed, &target);
        assert!(diffs.is_empty());
    }

    #[test]
    fn field_changed() {
        let transformed = make_transformed(vec![("name", Value::from("Acme Corp"))]);
        let target = make_target(vec![("name", Value::from("Acme"))]);

        let diffs = diff_fields(&transformed, &target);
        assert_eq!(diffs.len(), 1);
        assert_eq!(diffs[0].field, "name");
        assert_eq!(diffs[0].new_value, Value::from("Acme Corp"));
        assert_eq!(diffs[0].old_value, Value::from("Acme"));
    }

    #[test]
    fn new_field_not_in_target() {
        let transformed = make_transformed(vec![("description", Value::from("New description"))]);
        let target = make_target(vec![]);

        let diffs = diff_fields(&transformed, &target);
        assert_eq!(diffs.len(), 1);
        assert_eq!(diffs[0].field, "description");
        assert_eq!(diffs[0].new_value, Value::from("New description"));
        assert_eq!(diffs[0].old_value, Value::Null);
    }

    #[test]
    fn null_to_missing_is_no_diff() {
        // Transformed produces Null, target doesn't have the field → no diff
        let transformed = make_transformed(vec![("description", Value::Null)]);
        let target = make_target(vec![]);

        let diffs = diff_fields(&transformed, &target);
        assert!(diffs.is_empty());
    }

    #[test]
    fn null_replaces_existing_value() {
        // Transformed produces Null, target has a value → diff
        let transformed = make_transformed(vec![("name", Value::Null)]);
        let target = make_target(vec![("name", Value::from("Acme"))]);

        let diffs = diff_fields(&transformed, &target);
        assert_eq!(diffs.len(), 1);
        assert_eq!(diffs[0].field, "name");
        assert_eq!(diffs[0].new_value, Value::Null);
        assert_eq!(diffs[0].old_value, Value::from("Acme"));
    }

    #[test]
    fn cross_type_equality_no_diff() {
        // OptionSet(0) vs Int(0) → values_equal returns true → no diff
        let transformed = make_transformed(vec![("statecode", Value::Int(0))]);
        let target = make_target(vec![(
            "statecode",
            Value::OptionSet(OptionSetValue {
                value: 0,
                label: Some("Active".into()),
            }),
        )]);

        let diffs = diff_fields(&transformed, &target);
        assert!(diffs.is_empty());
    }

    #[test]
    fn multiple_diffs() {
        let transformed = make_transformed(vec![
            ("name", Value::from("New Name")),
            ("revenue", Value::Int(2000)),
            ("city", Value::from("NYC")),
        ]);
        let target = make_target(vec![
            ("name", Value::from("Old Name")),
            ("revenue", Value::Int(2000)), // same
            ("city", Value::from("LA")),
        ]);

        let diffs = diff_fields(&transformed, &target);
        assert_eq!(diffs.len(), 2);

        let field_names: Vec<&str> = diffs.iter().map(|d| d.field.as_str()).collect();
        assert!(field_names.contains(&"name"));
        assert!(field_names.contains(&"city"));
        assert!(!field_names.contains(&"revenue")); // unchanged
    }

    #[test]
    fn empty_transformed_no_diffs() {
        let transformed = make_transformed(vec![]);
        let target = make_target(vec![("name", Value::from("Acme"))]);

        let diffs = diff_fields(&transformed, &target);
        assert!(diffs.is_empty());
    }
}
