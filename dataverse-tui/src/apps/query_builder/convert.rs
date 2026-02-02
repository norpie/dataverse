//! Converts `QueryData` into dataverse-lib query types.

use dataverse_lib::api::query::odata::QueryBuilder as ODataQueryBuilder;
use dataverse_lib::api::query::{Direction, OrderBy};
use dataverse_lib::model::Entity;

use crate::widgets::filter_builder::{ConvertError as FilterConvertError, convert_filter};

use super::data::{QueryData, SortField};

/// Error converting `QueryData` to a library query.
#[derive(Debug, thiserror::Error)]
pub enum ConvertError {
    #[error("No entity selected")]
    NoEntity,
    #[error("{0}")]
    Filter(#[from] FilterConvertError),
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
    if let Some(filter) = convert_filter(&data.filter)? {
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
