//! GUID transform - generates a new random UUID.

use dataverse_lib::model::Value;
use uuid::Uuid;

use crate::apps::migration::engine::TransformResult;

/// Execute the GUID transform.
///
/// Generates a new random UUID. Ignores the current `#value`.
pub fn execute_guid() -> TransformResult {
    TransformResult::Value(Value::Guid(Uuid::new_v4()))
}
