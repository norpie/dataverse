//! Converts `QueryData` into dataverse-lib query types.

use dataverse_lib::DataverseClient;
use dataverse_lib::api::query::odata::QueryBuilder as ODataQueryBuilder;
use dataverse_lib::api::query::{Direction, Filter, OrderBy};
use dataverse_lib::model::{Entity, Value};

use super::data::{CondOp, FilterNode, QueryData, SortField};

/// Error converting `QueryData` to a library query.
#[derive(Debug, thiserror::Error)]
pub enum ConvertError {
    #[error("No entity selected")]
    NoEntity,
    #[error("String function operator requires a string value for field '{0}'")]
    NonStringValue(String),
}

/// Build an OData `QueryBuilder` from the user's `QueryData`.
pub fn build_query(data: &QueryData) -> Result<ODataQueryBuilder, ConvertError> {
    let entity = data.entity.as_ref().ok_or(ConvertError::NoEntity)?;
    let mut qb = ODataQueryBuilder::new(Entity::set(entity));

    // Select
    if !data.select.is_empty() {
        let fields: Vec<&str> = data.select.iter().map(|s| s.as_str()).collect();
        qb = qb.select(&fields);
    }

    // Filter
    if let Some(filter) = convert_filter_node(&data.filter)? {
        qb = qb.filter(filter);
    }

    // Order by
    if let Some(order) = convert_order_by(&data.order_by) {
        qb = qb.order_by(order);
    }

    // Top
    if let Some(top) = data.top {
        qb = qb.top(top as usize);
    }

    Ok(qb)
}

/// Convert a `FilterNode` tree to a library `Filter`.
///
/// Returns `None` for empty nodes or groups with no effective children.
fn convert_filter_node(node: &FilterNode) -> Result<Option<Filter>, ConvertError> {
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
            is_and, children, ..
        } => {
            let filters: Vec<Filter> = children
                .iter()
                .filter_map(|child| convert_filter_node(child).transpose())
                .collect::<Result<Vec<_>, _>>()?;

            if filters.is_empty() {
                return Ok(None);
            }

            if filters.len() == 1 {
                return Ok(filters.into_iter().next());
            }

            Ok(Some(if *is_and {
                Filter::and(filters)
            } else {
                Filter::or(filters)
            }))
        }
    }
}

/// Convert a single condition to a library `Filter`.
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

/// Extract a string from a `Value`, returning an error if not a string.
fn extract_string(field: &str, value: &Value) -> Result<String, ConvertError> {
    match value {
        Value::String(s) => Ok(s.clone()),
        _ => Err(ConvertError::NonStringValue(field.to_string())),
    }
}

/// Convert sort fields to an `OrderBy`.
///
/// Returns `None` if the list is empty.
fn convert_order_by(sorts: &[SortField]) -> Option<OrderBy> {
    let mut iter = sorts.iter();
    let first = iter.next()?;

    let mut order = match first.direction {
        Direction::Asc => OrderBy::asc(&first.field),
        Direction::Desc => OrderBy::desc(&first.field),
    };

    for sort in iter {
        order = match sort.direction {
            Direction::Asc => order.then_asc(&sort.field),
            Direction::Desc => order.then_desc(&sort.field),
        };
    }

    Some(order)
}
