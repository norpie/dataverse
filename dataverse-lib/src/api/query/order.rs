//! Ordering types for OData and FetchXML queries.

/// Sort direction for ordering results.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Ascending order (A-Z, 0-9).
    Asc,
    /// Descending order (Z-A, 9-0).
    Desc,
}

/// Specifies the ordering of query results.
///
/// Multiple fields can be chained together for secondary, tertiary, etc. sorting.
///
/// # Example
///
/// ```
/// use dataverse_lib::api::query::OrderBy;
///
/// // Single field ordering
/// let order = OrderBy::desc("revenue");
///
/// // Multiple field ordering
/// let order = OrderBy::desc("revenue")
///     .then_asc("name");
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct OrderBy {
    pub(crate) fields: Vec<(String, Direction)>,
}

impl OrderBy {
    /// Creates an ascending order on a field.
    pub fn asc(field: impl Into<String>) -> Self {
        Self {
            fields: vec![(field.into(), Direction::Asc)],
        }
    }

    /// Creates a descending order on a field.
    pub fn desc(field: impl Into<String>) -> Self {
        Self {
            fields: vec![(field.into(), Direction::Desc)],
        }
    }

    /// Adds a secondary ascending order on a field.
    pub fn then_asc(mut self, field: impl Into<String>) -> Self {
        self.fields.push((field.into(), Direction::Asc));
        self
    }

    /// Adds a secondary descending order on a field.
    pub fn then_desc(mut self, field: impl Into<String>) -> Self {
        self.fields.push((field.into(), Direction::Desc));
        self
    }

    /// Returns the ordered fields with their directions.
    pub fn fields(&self) -> &[(String, Direction)] {
        &self.fields
    }
}
