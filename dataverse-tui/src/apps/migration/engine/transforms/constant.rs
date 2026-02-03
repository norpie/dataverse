//! Constant transform - returns a static value.

use dataverse_lib::model::Value;

use crate::apps::migration::engine::TransformResult;

/// Execute the constant transform.
///
/// Simply returns the constant value unchanged. Ignores the current `#value`.
pub fn execute_constant(value: &Value) -> TransformResult {
    TransformResult::Value(value.clone())
}
