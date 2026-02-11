//! Converts FilterNode to dataverse-lib Filter.

use dataverse_lib::api::query::Filter;
use dataverse_lib::model::Value;

use super::types::{CondOp, FilterNode};

/// Error converting FilterNode to a library Filter.
#[derive(Debug, thiserror::Error)]
pub enum ConvertError {
    #[error("String function operator requires a string value for field '{0}'")]
    NonStringValue(String),
}

/// Convert a FilterNode tree to a library Filter.
///
/// Returns `None` for empty nodes or groups with no effective children.
pub fn convert_filter(node: &FilterNode) -> Result<Option<Filter>, ConvertError> {
    match node {
        FilterNode::Empty => Ok(None),
        FilterNode::Condition {
            field,
            operator,
            value,
            ..
        } => {
            if field.is_empty() {
                return Ok(None);
            }
            if operator.has_value() && matches!(value, Value::Null) {
                return Ok(None);
            }
            Ok(Some(convert_condition(field, *operator, value)?))
        }
        FilterNode::Group {
            is_and,
            is_negated,
            children,
            ..
        } => {
            let filters: Vec<Filter> = children
                .iter()
                .filter_map(|child| convert_filter(child).transpose())
                .collect::<Result<Vec<_>, _>>()?;

            if filters.is_empty() {
                return Ok(None);
            }

            let combined = if filters.len() == 1 {
                filters.into_iter().next().unwrap()
            } else if *is_and {
                Filter::and(filters)
            } else {
                Filter::or(filters)
            };

            if *is_negated {
                Ok(Some(Filter::Not(Box::new(combined))))
            } else {
                Ok(Some(combined))
            }
        }
    }
}

/// Convert a single condition to a library Filter.
fn convert_condition(field: &str, op: CondOp, value: &Value) -> Result<Filter, ConvertError> {
    match op {
        CondOp::Eq => Ok(Filter::eq(field, value.clone())),
        CondOp::Ne => Ok(Filter::ne(field, value.clone())),
        CondOp::Gt => Ok(Filter::gt(field, value.clone())),
        CondOp::Ge => Ok(Filter::ge(field, value.clone())),
        CondOp::Lt => Ok(Filter::lt(field, value.clone())),
        CondOp::Le => Ok(Filter::le(field, value.clone())),
        CondOp::Contains => {
            let s = extract_string(field, value)?;
            Ok(Filter::contains(field, s))
        }
        CondOp::StartsWith => {
            let s = extract_string(field, value)?;
            Ok(Filter::starts_with(field, s))
        }
        CondOp::EndsWith => {
            let s = extract_string(field, value)?;
            Ok(Filter::ends_with(field, s))
        }
        CondOp::IsNull => Ok(Filter::is_null(field)),
        CondOp::IsNotNull => Ok(Filter::is_not_null(field)),
    }
}

/// Extract a string from a Value, returning an error if not a string.
fn extract_string(field: &str, value: &Value) -> Result<String, ConvertError> {
    match value {
        Value::String(s) => Ok(s.clone()),
        _ => Err(ConvertError::NonStringValue(field.to_string())),
    }
}
