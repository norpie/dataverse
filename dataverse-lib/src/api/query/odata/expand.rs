//! OData $expand builder for nested navigation properties.

#[cfg(test)]
use crate::api::query::Filter;
use crate::api::query::ODataFilter;
use crate::api::query::OrderBy;

use super::url::odata_filter_to_string;
use super::url::order_to_odata;

/// Builder for constructing OData `$expand` clauses.
///
/// Supports nested query options within the expand, allowing filtering,
/// selecting, and ordering of related records.
///
/// # Example
///
/// ```ignore
/// let expand = ExpandBuilder::new("primarycontactid")
///     .select(&["fullname", "emailaddress1"])
///     .filter(Filter::eq("statecode", 0));
/// ```
#[derive(Debug, Clone)]
pub struct ExpandBuilder {
    /// The navigation property name to expand.
    navigation_property: String,
    /// Fields to select from the expanded entity.
    select: Vec<String>,
    /// Filter to apply to the expanded records.
    filter: Option<ODataFilter>,
    /// Ordering for the expanded records.
    order_by: Option<OrderBy>,
    /// Maximum number of expanded records to return.
    top: Option<usize>,
    /// Nested expands within this expand.
    expands: Vec<ExpandBuilder>,
}

impl ExpandBuilder {
    /// Creates a new expand builder for a navigation property.
    pub fn new(navigation_property: impl Into<String>) -> Self {
        Self {
            navigation_property: navigation_property.into(),
            select: Vec::new(),
            filter: None,
            order_by: None,
            top: None,
            expands: Vec::new(),
        }
    }

    /// Specifies which fields to select from the expanded entity.
    pub fn select(mut self, fields: &[&str]) -> Self {
        self.select = fields.iter().map(|s| (*s).to_string()).collect();
        self
    }

    /// Adds a filter condition to the expanded records.
    ///
    /// Accepts both [`Filter`] and [`ODataFilter`] (for negated filters).
    pub fn filter(mut self, filter: impl Into<ODataFilter>) -> Self {
        self.filter = Some(filter.into());
        self
    }

    /// Sets the ordering for expanded records.
    pub fn order_by(mut self, order: OrderBy) -> Self {
        self.order_by = Some(order);
        self
    }

    /// Limits the number of expanded records returned.
    pub fn top(mut self, n: usize) -> Self {
        self.top = Some(n);
        self
    }

    /// Adds a nested expand within this expand.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let expand = ExpandBuilder::new("primarycontactid")
    ///     .select(&["fullname"])
    ///     .expand("parentcustomerid_account", |e| {
    ///         e.select(&["name"])
    ///     });
    /// ```
    pub fn expand<F>(mut self, navigation_property: impl Into<String>, build: F) -> Self
    where
        F: FnOnce(ExpandBuilder) -> ExpandBuilder,
    {
        let nested = build(ExpandBuilder::new(navigation_property));
        self.expands.push(nested);
        self
    }

    /// Returns the navigation property name.
    pub fn navigation_property(&self) -> &str {
        &self.navigation_property
    }

    /// Converts this expand builder to an OData `$expand` clause.
    ///
    /// Returns the full expand expression including nested options.
    pub fn to_odata(&self) -> String {
        let mut parts = Vec::new();

        // $select
        if !self.select.is_empty() {
            parts.push(format!("$select={}", self.select.join(",")));
        }

        // $filter
        if let Some(ref filter) = self.filter {
            parts.push(format!("$filter={}", odata_filter_to_string(filter)));
        }

        // $orderby
        if let Some(ref order) = self.order_by {
            parts.push(format!("$orderby={}", order_to_odata(order)));
        }

        // $top
        if let Some(top) = self.top {
            parts.push(format!("$top={}", top));
        }

        // Nested $expand
        if !self.expands.is_empty() {
            let nested: Vec<_> = self.expands.iter().map(|e| e.to_odata()).collect();
            parts.push(format!("$expand={}", nested.join(",")));
        }

        if parts.is_empty() {
            self.navigation_property.clone()
        } else {
            format!("{}({})", self.navigation_property, parts.join(";"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_expand() {
        let expand = ExpandBuilder::new("primarycontactid");
        assert_eq!(expand.to_odata(), "primarycontactid");
    }

    #[test]
    fn test_expand_with_select() {
        let expand = ExpandBuilder::new("primarycontactid").select(&["fullname", "emailaddress1"]);
        assert_eq!(
            expand.to_odata(),
            "primarycontactid($select=fullname,emailaddress1)"
        );
    }

    #[test]
    fn test_expand_with_filter() {
        let expand =
            ExpandBuilder::new("contact_customer_accounts").filter(Filter::eq("statecode", 0i32));
        assert_eq!(
            expand.to_odata(),
            "contact_customer_accounts($filter=statecode eq 0)"
        );
    }

    #[test]
    fn test_expand_with_multiple_options() {
        let expand = ExpandBuilder::new("contact_customer_accounts")
            .select(&["fullname"])
            .filter(Filter::eq("statecode", 0i32))
            .order_by(OrderBy::asc("fullname"))
            .top(5);
        assert_eq!(
            expand.to_odata(),
            "contact_customer_accounts($select=fullname;$filter=statecode eq 0;$orderby=fullname asc;$top=5)"
        );
    }

    #[test]
    fn test_nested_expand() {
        let expand = ExpandBuilder::new("primarycontactid")
            .select(&["fullname"])
            .expand("parentcustomerid_account", |e| e.select(&["name"]));
        assert_eq!(
            expand.to_odata(),
            "primarycontactid($select=fullname;$expand=parentcustomerid_account($select=name))"
        );
    }
}
