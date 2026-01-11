//! FetchXML query builder.

use crate::api::query::Filter;
use crate::api::query::OrderBy;
use crate::error::Error;
use crate::model::Entity;
use crate::model::Record;
use crate::DataverseClient;

use super::link::LinkEntityBuilder;
use super::pages::FetchXmlPages;
use super::xml::attributes_to_fetchxml;
use super::xml::escape_xml;
use super::xml::filter_to_fetchxml;
use super::xml::order_to_fetchxml;

/// Builder for constructing FetchXML queries.
///
/// Use [`DataverseClient::fetch`] to create a fetch builder.
///
/// # Example
///
/// ```ignore
/// let mut pages = client.fetch(Entity::logical("account"))
///     .select(&["name", "revenue"])
///     .filter(Filter::gt("revenue", 1000000))
///     .order_by(OrderBy::desc("revenue"))
///     .into_async_iter();
///
/// while let Some(page) = pages.next().await {
///     let page = page?;
///     for record in page.records() {
///         println!("{:?}", record);
///     }
/// }
/// ```
pub struct FetchBuilder<'a> {
    client: &'a DataverseClient,
    entity: Entity,
    select: Vec<String>,
    filter: Option<Filter>,
    order_by: Option<OrderBy>,
    top: Option<usize>,
    page_size: Option<usize>,
    distinct: bool,
    links: Vec<LinkEntityBuilder>,
    include_count: bool,
}

impl<'a> FetchBuilder<'a> {
    /// Creates a new fetch builder for the given entity.
    pub(crate) fn new(client: &'a DataverseClient, entity: Entity) -> Self {
        Self {
            client,
            entity,
            select: Vec::new(),
            filter: None,
            order_by: None,
            top: None,
            page_size: None,
            distinct: false,
            links: Vec::new(),
            include_count: false,
        }
    }

    /// Specifies which fields to select.
    ///
    /// If not called, all fields are returned.
    pub fn select(mut self, fields: &[&str]) -> Self {
        self.select = fields.iter().map(|s| (*s).to_string()).collect();
        self
    }

    /// Adds a filter condition.
    pub fn filter(mut self, filter: Filter) -> Self {
        self.filter = Some(filter);
        self
    }

    /// Sets the ordering of results.
    pub fn order_by(mut self, order: OrderBy) -> Self {
        self.order_by = Some(order);
        self
    }

    /// Limits the total number of records returned.
    pub fn top(mut self, n: usize) -> Self {
        self.top = Some(n);
        self
    }

    /// Sets the page size for pagination.
    ///
    /// This controls how many records are returned per page. The default is
    /// determined by the server (typically 5000).
    pub fn page_size(mut self, size: usize) -> Self {
        self.page_size = Some(size);
        self
    }

    /// Sets whether to return only distinct records.
    pub fn distinct(mut self, distinct: bool) -> Self {
        self.distinct = distinct;
        self
    }

    /// Adds a link entity (join) to the query.
    ///
    /// # Example
    ///
    /// ```ignore
    /// client.fetch(Entity::logical("account"))
    ///     .select(&["name"])
    ///     .link_entity("contact", "contactid", "primarycontactid", |link| {
    ///         link.alias("pc")
    ///             .select(&["fullname"])
    ///             .link_type(LinkType::Outer)
    ///     });
    /// ```
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

    /// Includes the total count of matching records in the response.
    pub fn include_count(mut self) -> Self {
        self.include_count = true;
        self
    }

    /// Returns the entity logical name.
    fn entity_logical_name(&self) -> &str {
        match &self.entity {
            Entity::Set(name) => name,
            Entity::Logical(name) => name,
        }
    }

    /// Builds the FetchXML string.
    pub(crate) fn build_fetchxml(&self) -> String {
        let entity_name = self.entity_logical_name();

        // Build fetch attributes
        let mut fetch_attrs = vec![
            r#"version="1.0""#.to_string(),
            r#"output-format="xml-platform""#.to_string(),
            r#"mapping="logical""#.to_string(),
        ];

        if self.distinct {
            fetch_attrs.push(r#"distinct="true""#.to_string());
        } else {
            fetch_attrs.push(r#"distinct="false""#.to_string());
        }

        if let Some(top) = self.top {
            fetch_attrs.push(format!(r#"top="{}""#, top));
        }

        if let Some(count) = self.page_size {
            fetch_attrs.push(format!(r#"count="{}""#, count));
        }

        if self.include_count {
            fetch_attrs.push(r#"returntotalrecordcount="true""#.to_string());
        }

        // Build entity content
        let mut entity_content = String::new();

        // Attributes
        if !self.select.is_empty() {
            entity_content.push_str(&attributes_to_fetchxml(&self.select));
        }

        // Filter
        if let Some(ref filter) = self.filter {
            let filter_xml = filter_to_fetchxml(filter);
            // Wrap in filter element if it's just conditions
            if !filter_xml.starts_with("<filter") {
                entity_content.push_str(&format!(r#"<filter type="and">{}</filter>"#, filter_xml));
            } else {
                entity_content.push_str(&filter_xml);
            }
        }

        // Order
        if let Some(ref order) = self.order_by {
            entity_content.push_str(&order_to_fetchxml(order));
        }

        // Link entities
        for link in &self.links {
            entity_content.push_str(&link.to_fetchxml());
        }

        format!(
            r#"<fetch {}><entity name="{}">{}</entity></fetch>"#,
            fetch_attrs.join(" "),
            escape_xml(entity_name),
            entity_content
        )
    }

    /// Returns the page size, if set.
    pub(crate) fn page_size_value(&self) -> Option<usize> {
        self.page_size
    }

    /// Returns a reference to the entity.
    pub(crate) fn entity(&self) -> &Entity {
        &self.entity
    }

    /// Returns a reference to the client.
    pub(crate) fn client(&self) -> &'a DataverseClient {
        self.client
    }

    /// Resolves the entity to its entity set name.
    async fn resolve_entity_set(&self) -> Result<String, Error> {
        match &self.entity {
            Entity::Set(name) => Ok(name.clone()),
            Entity::Logical(logical_name) => {
                self.client.resolve_entity_set_name(logical_name).await
            }
        }
    }

    /// Executes the query and returns the first page of results.
    ///
    /// Use `into_async_iter()` to iterate over all pages.
    pub async fn execute(self) -> Result<Vec<Record>, Error> {
        let mut pages = self.into_async_iter();
        match pages.next().await {
            Some(Ok(page)) => Ok(page.into_records()),
            Some(Err(e)) => Err(e),
            None => Ok(Vec::new()),
        }
    }

    /// Executes the query and returns the first matching record.
    pub async fn first(self) -> Result<Option<Record>, Error> {
        // Optimize by limiting to 1 record
        let builder = Self {
            top: Some(1),
            ..self
        };
        let records = builder.execute().await?;
        Ok(records.into_iter().next())
    }

    /// Converts this fetch builder into an async iterator over pages.
    pub fn into_async_iter(self) -> FetchXmlPages<'a> {
        FetchXmlPages::new(self)
    }
}
