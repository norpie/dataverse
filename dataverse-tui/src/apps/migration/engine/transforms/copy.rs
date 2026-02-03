//! Copy transform - copies a value from the source record.

use dataverse_lib::model::Entity;
use dataverse_lib::model::Record;
use dataverse_lib::model::Value;

use crate::apps::migration::engine::TransformError;
use crate::apps::migration::engine::TransformResult;

/// Execute the copy transform.
///
/// Copies a value from the source record using dot-notation path traversal.
/// Lookups are always expanded as nested Records, so paths like
/// `"primarycontactid.parentaccountid.name"` traverse through Records.
///
/// # Type Annotation
///
/// When traversing through a lookup (nested Record), sets `#type` to the
/// entity of the last traversed Record. This allows downstream transforms
/// to know what entity type the value came from.
///
/// # Returns
///
/// - `(TransformResult::Value, Some(entity))` - value with type from traversed lookup
/// - `(TransformResult::Value, None)` - value from top-level field (no lookup traversal)
/// - `(TransformResult::Error(PathNotFound), None)` - path doesn't exist
pub fn execute_copy(path: &str, source_record: &Record) -> (TransformResult, Option<Entity>) {
    let segments: Vec<&str> = path.split('.').collect();

    if segments.is_empty() {
        return (
            TransformResult::Error(TransformError::path_not_found(path)),
            None,
        );
    }

    let mut current_record = source_record;
    let mut last_entity: Option<Entity> = None;

    // Traverse through all segments except the last
    for &segment in &segments[..segments.len() - 1] {
        match current_record.get(segment) {
            Some(Value::Record(nested)) => {
                last_entity = Some(nested.entity().clone());
                current_record = nested;
            }
            Some(_) => {
                // Not a Record, can't traverse further
                return (
                    TransformResult::Error(TransformError::path_not_found(path)),
                    None,
                );
            }
            None => {
                return (
                    TransformResult::Error(TransformError::path_not_found(path)),
                    None,
                );
            }
        }
    }

    // Get the final value
    let final_segment = segments[segments.len() - 1];
    match current_record.get(final_segment) {
        Some(value) => (TransformResult::Value(value.clone()), last_entity),
        None => (
            TransformResult::Error(TransformError::path_not_found(path)),
            None,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_record() -> Record {
        // account with expanded primarycontactid -> contact with expanded parentcustomerid -> account
        let grandparent = Record::new("account")
            .set("name", "Parent Corp")
            .set("accountid", "grand-guid");

        let contact = Record::new("contact")
            .set("fullname", "John Smith")
            .set("parentcustomerid", Value::Record(Box::new(grandparent)));

        Record::new("account")
            .set("name", "Contoso")
            .set("revenue", 1_000_000i64)
            .set("primarycontactid", Value::Record(Box::new(contact)))
    }

    #[test]
    fn copies_top_level_field() {
        let record = make_record();
        let (result, value_type) = execute_copy("name", &record);

        assert!(matches!(result, TransformResult::Value(Value::String(s)) if s == "Contoso"));
        assert!(value_type.is_none()); // No lookup traversal
    }

    #[test]
    fn copies_through_one_lookup() {
        let record = make_record();
        let (result, value_type) = execute_copy("primarycontactid.fullname", &record);

        assert!(matches!(result, TransformResult::Value(Value::String(s)) if s == "John Smith"));
        assert_eq!(value_type, Some(Entity::logical("contact")));
    }

    #[test]
    fn copies_through_two_lookups() {
        let record = make_record();
        let (result, value_type) = execute_copy("primarycontactid.parentcustomerid.name", &record);

        assert!(matches!(result, TransformResult::Value(Value::String(s)) if s == "Parent Corp"));
        assert_eq!(value_type, Some(Entity::logical("account")));
    }

    #[test]
    fn missing_field_returns_error() {
        let record = make_record();
        let (result, _) = execute_copy("nonexistent", &record);

        assert!(matches!(
            result,
            TransformResult::Error(TransformError::PathNotFound { .. })
        ));
    }

    #[test]
    fn missing_nested_field_returns_error() {
        let record = make_record();
        let (result, _) = execute_copy("primarycontactid.nonexistent", &record);

        assert!(matches!(
            result,
            TransformResult::Error(TransformError::PathNotFound { .. })
        ));
    }

    #[test]
    fn traverse_non_record_returns_error() {
        let record = make_record();
        let (result, _) = execute_copy("name.something", &record);

        assert!(matches!(
            result,
            TransformResult::Error(TransformError::PathNotFound { .. })
        ));
    }
}
