//! Link entity builder for FetchXML joins.

use crate::api::query::Filter;
use crate::api::query::OrderBy;

use super::xml::attributes_to_fetchxml;
use super::xml::escape_xml;
use super::xml::filter_to_fetchxml;
use super::xml::order_to_fetchxml;

/// The type of join for a link-entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LinkType {
    /// Inner join (default) - only returns records where the link exists.
    #[default]
    Inner,
    /// Outer join - returns all records, with null for missing links.
    Outer,
}

/// Builder for FetchXML `<link-entity>` elements.
///
/// Link entities allow joining related tables in FetchXML queries.
///
/// # Example
///
/// ```ignore
/// client.fetch(Entity::logical("account"))
///     .link_entity("contact", "contactid", "primarycontactid", |link| {
///         link.alias("pc")
///             .select(&["fullname", "emailaddress1"])
///             .link_type(LinkType::Outer)
///     });
/// ```
#[derive(Debug, Clone)]
pub struct LinkEntityBuilder {
    /// The logical name of the linked entity.
    entity_name: String,
    /// The attribute on the linked entity to join from.
    from: String,
    /// The attribute on the parent entity to join to.
    to: String,
    /// The type of join (inner or outer).
    link_type: LinkType,
    /// Alias for the linked entity.
    alias: Option<String>,
    /// Attributes to select from the linked entity.
    select: Vec<String>,
    /// Filter conditions for the linked entity.
    filter: Option<Filter>,
    /// Ordering for records from the linked entity.
    order_by: Option<OrderBy>,
    /// Nested link entities.
    links: Vec<LinkEntityBuilder>,
}

impl LinkEntityBuilder {
    /// Creates a new link entity builder.
    ///
    /// # Arguments
    ///
    /// * `entity_name` - The logical name of the entity to link to
    /// * `from` - The attribute on the linked entity to join from
    /// * `to` - The attribute on the parent entity to join to
    pub fn new(
        entity_name: impl Into<String>,
        from: impl Into<String>,
        to: impl Into<String>,
    ) -> Self {
        Self {
            entity_name: entity_name.into(),
            from: from.into(),
            to: to.into(),
            link_type: LinkType::default(),
            alias: None,
            select: Vec::new(),
            filter: None,
            order_by: None,
            links: Vec::new(),
        }
    }

    /// Sets the link type (inner or outer join).
    pub fn link_type(mut self, link_type: LinkType) -> Self {
        self.link_type = link_type;
        self
    }

    /// Sets an alias for the linked entity.
    ///
    /// Aliases are used to distinguish attributes from linked entities
    /// in the result set.
    pub fn alias(mut self, alias: impl Into<String>) -> Self {
        self.alias = Some(alias.into());
        self
    }

    /// Specifies which attributes to select from the linked entity.
    pub fn select(mut self, fields: &[&str]) -> Self {
        self.select = fields.iter().map(|s| (*s).to_string()).collect();
        self
    }

    /// Adds a filter condition for the linked entity.
    pub fn filter(mut self, filter: Filter) -> Self {
        self.filter = Some(filter);
        self
    }

    /// Sets the ordering for records from the linked entity.
    pub fn order_by(mut self, order: OrderBy) -> Self {
        self.order_by = Some(order);
        self
    }

    /// Adds a nested link entity.
    ///
    /// This allows for multi-level joins in FetchXML queries.
    pub fn link_entity<F>(
        mut self,
        entity_name: impl Into<String>,
        from: impl Into<String>,
        to: impl Into<String>,
        build: F,
    ) -> Self
    where
        F: FnOnce(LinkEntityBuilder) -> LinkEntityBuilder,
    {
        let link = build(LinkEntityBuilder::new(entity_name, from, to));
        self.links.push(link);
        self
    }

    /// Converts this link entity to FetchXML.
    pub fn to_fetchxml(&self) -> String {
        let link_type_str = match self.link_type {
            LinkType::Inner => "inner",
            LinkType::Outer => "outer",
        };

        let alias_attr = self
            .alias
            .as_ref()
            .map(|a| format!(r#" alias="{}""#, escape_xml(a)))
            .unwrap_or_default();

        let mut content = String::new();

        // Attributes
        if !self.select.is_empty() {
            content.push_str(&attributes_to_fetchxml(&self.select));
        }

        // Filter
        if let Some(ref filter) = self.filter {
            let filter_xml = filter_to_fetchxml(filter);
            // Wrap in filter element if it's just conditions
            if !filter_xml.starts_with("<filter") {
                content.push_str(&format!(r#"<filter type="and">{}</filter>"#, filter_xml));
            } else {
                content.push_str(&filter_xml);
            }
        }

        // Order
        if let Some(ref order) = self.order_by {
            content.push_str(&order_to_fetchxml(order));
        }

        // Nested links
        for link in &self.links {
            content.push_str(&link.to_fetchxml());
        }

        format!(
            r#"<link-entity name="{}" from="{}" to="{}" link-type="{}"{}>{}</link-entity>"#,
            escape_xml(&self.entity_name),
            escape_xml(&self.from),
            escape_xml(&self.to),
            link_type_str,
            alias_attr,
            content
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_link() {
        let link = LinkEntityBuilder::new("contact", "contactid", "primarycontactid");
        assert_eq!(
            link.to_fetchxml(),
            r#"<link-entity name="contact" from="contactid" to="primarycontactid" link-type="inner"></link-entity>"#
        );
    }

    #[test]
    fn test_link_with_options() {
        let link = LinkEntityBuilder::new("contact", "contactid", "primarycontactid")
            .alias("pc")
            .link_type(LinkType::Outer)
            .select(&["fullname", "emailaddress1"]);

        assert_eq!(
            link.to_fetchxml(),
            r#"<link-entity name="contact" from="contactid" to="primarycontactid" link-type="outer" alias="pc"><attribute name="fullname"/><attribute name="emailaddress1"/></link-entity>"#
        );
    }

    #[test]
    fn test_link_with_filter() {
        let link = LinkEntityBuilder::new("contact", "contactid", "primarycontactid")
            .filter(Filter::eq("statecode", 0i32));

        assert!(link.to_fetchxml().contains(r#"<filter type="and">"#));
        assert!(link.to_fetchxml().contains(r#"operator="eq""#));
    }

    #[test]
    fn test_nested_links() {
        let link = LinkEntityBuilder::new("contact", "contactid", "primarycontactid")
            .link_entity("account", "accountid", "parentcustomerid", |nested| {
                nested.select(&["name"])
            });

        let xml = link.to_fetchxml();
        assert!(xml.contains(r#"<link-entity name="contact""#));
        assert!(xml.contains(r#"<link-entity name="account""#));
    }
}
