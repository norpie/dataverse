//! Filter types for OData and FetchXML queries.

use crate::model::Value;

/// A filter condition for querying records.
///
/// Filters can be combined using logical operators (`And`, `Or`, `Not`) to build
/// complex query conditions. Both OData and FetchXML queries use this same type.
///
/// # Example
///
/// ```
/// use dataverse_lib::api::query::Filter;
///
/// // Simple equality filter
/// let filter = Filter::eq("statecode", 0);
///
/// // Combined filter
/// let filter = Filter::and([
///     Filter::eq("statecode", 0),
///     Filter::gt("revenue", 1_000_000),
/// ]);
///
/// // Using combinators
/// let filter = Filter::eq("statecode", 0)
///     .and_also(Filter::gt("revenue", 1_000_000));
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum Filter {
    /// Equality: `field eq value`
    Eq(String, Value),
    /// Not equal: `field ne value`
    Ne(String, Value),
    /// Greater than: `field gt value`
    Gt(String, Value),
    /// Greater than or equal: `field ge value`
    Ge(String, Value),
    /// Less than: `field lt value`
    Lt(String, Value),
    /// Less than or equal: `field le value`
    Le(String, Value),
    /// Contains substring: `contains(field, 'value')`
    Contains(String, String),
    /// Starts with: `startswith(field, 'value')`
    StartsWith(String, String),
    /// Ends with: `endswith(field, 'value')`
    EndsWith(String, String),
    /// Is null: `field eq null`
    IsNull(String),
    /// Is not null: `field ne null`
    IsNotNull(String),
    /// Logical AND of multiple filters.
    And(Vec<Filter>),
    /// Logical OR of multiple filters.
    Or(Vec<Filter>),
    /// Logical NOT of a filter.
    Not(Box<Filter>),
    /// Raw OData/FetchXML filter string (escape hatch).
    Raw(String),
}

impl Filter {
    /// Creates an equality filter: `field eq value`.
    pub fn eq(field: impl Into<String>, value: impl Into<Value>) -> Self {
        Filter::Eq(field.into(), value.into())
    }

    /// Creates a not-equal filter: `field ne value`.
    pub fn ne(field: impl Into<String>, value: impl Into<Value>) -> Self {
        Filter::Ne(field.into(), value.into())
    }

    /// Creates a greater-than filter: `field gt value`.
    pub fn gt(field: impl Into<String>, value: impl Into<Value>) -> Self {
        Filter::Gt(field.into(), value.into())
    }

    /// Creates a greater-than-or-equal filter: `field ge value`.
    pub fn ge(field: impl Into<String>, value: impl Into<Value>) -> Self {
        Filter::Ge(field.into(), value.into())
    }

    /// Creates a less-than filter: `field lt value`.
    pub fn lt(field: impl Into<String>, value: impl Into<Value>) -> Self {
        Filter::Lt(field.into(), value.into())
    }

    /// Creates a less-than-or-equal filter: `field le value`.
    pub fn le(field: impl Into<String>, value: impl Into<Value>) -> Self {
        Filter::Le(field.into(), value.into())
    }

    /// Creates a contains filter: `contains(field, 'value')`.
    pub fn contains(field: impl Into<String>, value: impl Into<String>) -> Self {
        Filter::Contains(field.into(), value.into())
    }

    /// Creates a starts-with filter: `startswith(field, 'value')`.
    pub fn starts_with(field: impl Into<String>, value: impl Into<String>) -> Self {
        Filter::StartsWith(field.into(), value.into())
    }

    /// Creates an ends-with filter: `endswith(field, 'value')`.
    pub fn ends_with(field: impl Into<String>, value: impl Into<String>) -> Self {
        Filter::EndsWith(field.into(), value.into())
    }

    /// Creates an is-null filter: `field eq null`.
    pub fn is_null(field: impl Into<String>) -> Self {
        Filter::IsNull(field.into())
    }

    /// Creates an is-not-null filter: `field ne null`.
    pub fn is_not_null(field: impl Into<String>) -> Self {
        Filter::IsNotNull(field.into())
    }

    /// Creates a logical AND of multiple filters.
    pub fn and(filters: impl IntoIterator<Item = Filter>) -> Self {
        Filter::And(filters.into_iter().collect())
    }

    /// Creates a logical OR of multiple filters.
    pub fn or(filters: impl IntoIterator<Item = Filter>) -> Self {
        Filter::Or(filters.into_iter().collect())
    }

    /// Creates a logical NOT of a filter.
    pub fn not(filter: Filter) -> Self {
        Filter::Not(Box::new(filter))
    }

    /// Creates a raw filter string (escape hatch).
    ///
    /// Use this when you need to write a filter that isn't supported by the
    /// typed API. The string is passed through as-is.
    pub fn raw(filter: impl Into<String>) -> Self {
        Filter::Raw(filter.into())
    }

    /// Combines this filter with another using logical AND.
    ///
    /// # Example
    ///
    /// ```
    /// use dataverse_lib::api::query::Filter;
    ///
    /// let filter = Filter::eq("statecode", 0)
    ///     .and_also(Filter::gt("revenue", 1_000_000));
    /// ```
    pub fn and_also(self, other: Filter) -> Self {
        match self {
            Filter::And(mut filters) => {
                filters.push(other);
                Filter::And(filters)
            }
            _ => Filter::And(vec![self, other]),
        }
    }

    /// Combines this filter with another using logical OR.
    ///
    /// # Example
    ///
    /// ```
    /// use dataverse_lib::api::query::Filter;
    ///
    /// let filter = Filter::eq("statecode", 0)
    ///     .or_else(Filter::eq("statecode", 1));
    /// ```
    pub fn or_else(self, other: Filter) -> Self {
        match self {
            Filter::Or(mut filters) => {
                filters.push(other);
                Filter::Or(filters)
            }
            _ => Filter::Or(vec![self, other]),
        }
    }
}
