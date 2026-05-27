//! Shared utility functions for the transform engine.

use std::sync::Arc;

use dataverse_lib::model::Record;
use dataverse_lib::model::Value;

/// Check equality between two values, with cross-type flexibility.
///
/// Handles numeric coercion (Int↔Long, OptionSet↔Int) and
/// approximate float comparison.
pub fn values_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Null, Value::Null) => true,
        // Null ↔ empty string: semantically the same in Dataverse
        (Value::Null, Value::String(s)) | (Value::String(s), Value::Null) => s.is_empty(),
        (Value::Bool(a), Value::Bool(b)) => a == b,
        (Value::Int(a), Value::Int(b)) => a == b,
        (Value::Long(a), Value::Long(b)) => a == b,
        (Value::Int(a), Value::Long(b)) => (*a as i64) == *b,
        (Value::Long(a), Value::Int(b)) => *a == (*b as i64),
        (Value::Float(a), Value::Float(b)) => (a - b).abs() < f64::EPSILON,
        // Float ↔ Int/Long/OptionSet coercion
        (Value::Float(a), Value::Int(b)) => (*a - *b as f64).abs() < f64::EPSILON,
        (Value::Int(a), Value::Float(b)) => (*a as f64 - *b).abs() < f64::EPSILON,
        (Value::Float(a), Value::Long(b)) => (*a - *b as f64).abs() < f64::EPSILON,
        (Value::Long(a), Value::Float(b)) => (*a as f64 - *b).abs() < f64::EPSILON,
        (Value::Float(a), Value::OptionSet(b)) => (*a - b.value as f64).abs() < f64::EPSILON,
        (Value::OptionSet(a), Value::Float(b)) => (a.value as f64 - *b).abs() < f64::EPSILON,
        (Value::Decimal(a), Value::Decimal(b)) => a == b,
        // Decimal ↔ Int/Long coercion
        (Value::Decimal(a), Value::Int(b)) => {
            a.is_integer() && a == &rust_decimal::Decimal::from(*b)
        }
        (Value::Int(a), Value::Decimal(b)) => {
            b.is_integer() && &rust_decimal::Decimal::from(*a) == b
        }
        (Value::Decimal(a), Value::Long(b)) => {
            a.is_integer() && a == &rust_decimal::Decimal::from(*b)
        }
        (Value::Long(a), Value::Decimal(b)) => {
            b.is_integer() && &rust_decimal::Decimal::from(*a) == b
        }
        (Value::Decimal(a), Value::OptionSet(b)) => {
            a.is_integer() && a == &rust_decimal::Decimal::from(b.value)
        }
        (Value::OptionSet(a), Value::Decimal(b)) => {
            b.is_integer() && &rust_decimal::Decimal::from(a.value) == b
        }
        (Value::String(a), Value::String(b)) => a == b,
        (Value::Guid(a), Value::Guid(b)) => a == b,
        (Value::DateTime(a), Value::DateTime(b)) => a == b,
        (Value::Money(a), Value::Money(b)) => a.0 == b.0,
        (Value::OptionSet(a), Value::OptionSet(b)) => a.value == b.value,
        (Value::OptionSet(a), Value::Int(b)) => a.value == *b,
        (Value::Int(a), Value::OptionSet(b)) => *a == b.value,
        // Lookup comparisons — compare by GUID regardless of read/write format
        (Value::EntityReference(a), Value::EntityReference(b)) => a.id == b.id,
        (Value::EntityBinding(a), Value::EntityBinding(b)) => a.id == b.id,
        (Value::EntityReference(a), Value::EntityBinding(b)) => Some(a.id) == b.id,
        (Value::EntityBinding(a), Value::EntityReference(b)) => a.id == Some(b.id),
        _ => false,
    }
}

/// Traverse a dotted path through a record, returning the value at the end.
///
/// Simpler than `traverse_record` in resolve.rs — no TransformResult/entity tracking,
/// just walks through `Value::Record(nested)` following each segment.
///
/// Uses case-insensitive fallback for field lookups to handle the mismatch between
/// user-configured logical names (lowercase, e.g., `nrq_projectid`) and OData
/// navigation property keys (SchemaName/PascalCase, e.g., `nrq_ProjectId`).
///
/// # Examples
///
/// - `traverse_path(record, "name")` → `record.get("name")`
/// - `traverse_path(record, "contact.emailaddress1")` → walks into nested `contact` record
pub fn traverse_path<'a>(record: &'a Record, path: &str) -> Option<&'a Value> {
    let segments: Vec<&str> = path.split('.').collect();

    if segments.is_empty() {
        return None;
    }

    if segments.len() == 1 {
        return record_get_insensitive(record, segments[0]);
    }

    // Walk through nested records
    let mut current_record = record;
    for (i, segment) in segments.iter().enumerate() {
        if i == segments.len() - 1 {
            // Last segment — return the value
            return record_get_insensitive(current_record, segment);
        }

        // Intermediate segment — must be a nested Record
        match record_get_insensitive(current_record, segment) {
            Some(Value::Record(nested)) => {
                current_record = nested;
            }
            _ => return None,
        }
    }

    None
}

/// Get a field from a record, falling back to case-insensitive matching.
///
/// Dataverse OData responses use SchemaName (PascalCase) for navigation property keys
/// (e.g., `nrq_ProjectId`), but user configuration uses logical names (lowercase,
/// e.g., `nrq_projectid`). This function tries an exact match first, then falls back
/// to case-insensitive comparison.
pub fn record_get_insensitive<'a>(record: &'a Record, field: &str) -> Option<&'a Value> {
    // Fast path: exact match
    if let Some(v) = record.get(field) {
        return Some(v);
    }
    // Fallback: case-insensitive search for navigation property keys
    let field_lower = field.to_lowercase();
    record.fields().iter().find_map(|(k, v)| {
        if k.to_lowercase() == field_lower {
            Some(v)
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use dataverse_lib::model::Entity;

    fn make_record(entity: &str, fields: Vec<(&str, Value)>) -> Record {
        let mut record = Record::new(Entity::logical(entity));
        for (field, value) in fields {
            record = record.set(field, value);
        }
        record
    }

    #[test]
    fn traverse_single_segment() {
        let record = make_record("account", vec![("name", Value::from("Acme"))]);
        assert_eq!(traverse_path(&record, "name"), Some(&Value::from("Acme")));
    }

    #[test]
    fn traverse_single_segment_missing() {
        let record = make_record("account", vec![]);
        assert_eq!(traverse_path(&record, "name"), None);
    }

    #[test]
    fn traverse_dotted_path() {
        let nested = make_record(
            "contact",
            vec![("emailaddress1", Value::from("alice@example.com"))],
        );
        let record = make_record(
            "account",
            vec![("primarycontactid", Value::Record(Arc::new(nested)))],
        );
        assert_eq!(
            traverse_path(&record, "primarycontactid.emailaddress1"),
            Some(&Value::from("alice@example.com"))
        );
    }

    #[test]
    fn traverse_dotted_path_missing_intermediate() {
        let record = make_record("account", vec![("name", Value::from("Acme"))]);
        assert_eq!(
            traverse_path(&record, "primarycontactid.emailaddress1"),
            None
        );
    }

    #[test]
    fn traverse_dotted_path_non_record_intermediate() {
        let record = make_record("account", vec![("name", Value::from("Acme"))]);
        // "name" is a string, not a record — can't traverse further
        assert_eq!(traverse_path(&record, "name.something"), None);
    }

    #[test]
    fn traverse_three_level_path() {
        let inner = make_record("account", vec![("name", Value::from("ParentCo"))]);
        let middle = make_record(
            "contact",
            vec![("parentcustomerid", Value::Record(Arc::new(inner)))],
        );
        let record = make_record(
            "account",
            vec![("primarycontactid", Value::Record(Arc::new(middle)))],
        );
        assert_eq!(
            traverse_path(&record, "primarycontactid.parentcustomerid.name"),
            Some(&Value::from("ParentCo"))
        );
    }

    #[test]
    fn traverse_dotted_path_missing_leaf() {
        let nested = make_record("contact", vec![("fullname", Value::from("Alice"))]);
        let record = make_record(
            "account",
            vec![("primarycontactid", Value::Record(Arc::new(nested)))],
        );
        // Field exists on nested record but we're asking for a different field
        assert_eq!(
            traverse_path(&record, "primarycontactid.emailaddress1"),
            None
        );
    }
}
