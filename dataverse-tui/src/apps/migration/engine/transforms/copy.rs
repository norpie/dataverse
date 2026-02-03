//! Copy transform - copies a value from the source record.

use dataverse_lib::model::Entity;
use dataverse_lib::model::Record;
use dataverse_lib::model::Value;

use crate::apps::migration::engine::TransformError;
use crate::apps::migration::engine::TransformResult;

/// A parsed path segment.
struct Segment<'a> {
    /// The field name (without `?` suffix).
    name: &'a str,
    /// Whether this segment allows null propagation (`?` suffix).
    optional: bool,
}

impl<'a> Segment<'a> {
    fn parse(s: &'a str) -> Self {
        if let Some(name) = s.strip_suffix('?') {
            Self {
                name,
                optional: true,
            }
        } else {
            Self {
                name: s,
                optional: false,
            }
        }
    }
}

/// Execute the copy transform.
///
/// Copies a value from the source record using dot-notation path traversal.
/// Lookups are always expanded as nested Records, so paths like
/// `"primarycontactid.parentaccountid.name"` traverse through Records.
///
/// # Optional Chaining
///
/// Use `?` suffix for null-safe traversal:
/// - `"primarycontactid.name"` → errors if `primarycontactid` is null
/// - `"primarycontactid?.name"` → returns `Value::Null` if `primarycontactid` is null
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
/// - `(TransformResult::Error(NullInPath), None)` - null lookup without `?`
pub fn execute_copy(path: &str, source_record: &Record) -> (TransformResult, Option<Entity>) {
    let segments: Vec<Segment> = path.split('.').map(Segment::parse).collect();

    if segments.is_empty() {
        return (
            TransformResult::Error(TransformError::path_not_found(path)),
            None,
        );
    }

    let mut current_record = source_record;
    let mut last_entity: Option<Entity> = None;

    // Traverse through all segments except the last
    for segment in &segments[..segments.len() - 1] {
        match current_record.get(segment.name) {
            Some(Value::Record(nested)) => {
                last_entity = Some(nested.entity().clone());
                current_record = nested;
            }
            Some(Value::Null) => {
                // Null lookup - check if optional
                if segment.optional {
                    return (TransformResult::Value(Value::Null), None);
                } else {
                    return (
                        TransformResult::Error(TransformError::null_in_path(segment.name)),
                        None,
                    );
                }
            }
            Some(_) => {
                // Not a Record or Null, can't traverse further
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
    let final_segment = &segments[segments.len() - 1];
    match current_record.get(final_segment.name) {
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
            .set("secondarycontactid", Value::Null) // null lookup for testing
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

    #[test]
    fn null_lookup_without_optional_returns_error() {
        let record = make_record();
        let (result, _) = execute_copy("secondarycontactid.fullname", &record);

        assert!(matches!(
            result,
            TransformResult::Error(TransformError::NullInPath { .. })
        ));
    }

    #[test]
    fn null_lookup_with_optional_returns_null() {
        let record = make_record();
        let (result, value_type) = execute_copy("secondarycontactid?.fullname", &record);

        assert!(matches!(result, TransformResult::Value(Value::Null)));
        assert!(value_type.is_none());
    }

    #[test]
    fn optional_on_non_null_lookup_works() {
        let record = make_record();
        let (result, value_type) = execute_copy("primarycontactid?.fullname", &record);

        assert!(matches!(result, TransformResult::Value(Value::String(s)) if s == "John Smith"));
        assert_eq!(value_type, Some(Entity::logical("contact")));
    }

    #[test]
    fn chained_optional_with_null_in_middle() {
        // contact exists but parentcustomerid is null
        let contact = Record::new("contact")
            .set("fullname", "Jane Doe")
            .set("parentcustomerid", Value::Null);

        let record = Record::new("account")
            .set("name", "Test")
            .set("primarycontactid", Value::Record(Box::new(contact)));

        // Without ? on parentcustomerid - should error
        let (result, _) = execute_copy("primarycontactid.parentcustomerid.name", &record);
        assert!(matches!(
            result,
            TransformResult::Error(TransformError::NullInPath { .. })
        ));

        // With ? on parentcustomerid - should return null
        let (result, _) = execute_copy("primarycontactid.parentcustomerid?.name", &record);
        assert!(matches!(result, TransformResult::Value(Value::Null)));
    }
}
