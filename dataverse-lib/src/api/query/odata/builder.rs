//! OData query builder.

use crate::api::query::ODataFilter;
use crate::api::query::OrderBy;
use crate::error::Error;
use crate::model::Entity;
use crate::model::Record;
use crate::DataverseClient;

use super::expand::ExpandBuilder;
use super::pages::ODataPages;
use super::url::odata_filter_to_string;
use super::url::order_to_odata;

/// Builder for constructing OData queries.
///
/// Use [`DataverseClient::query`] to create a query builder.
///
/// # Example
///
/// ```ignore
/// let mut pages = client.query(Entity::logical("account"))
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
pub struct QueryBuilder<'a> {
    client: &'a DataverseClient,
    entity: Entity,
    select: Vec<String>,
    filter: Option<ODataFilter>,
    order_by: Option<OrderBy>,
    top: Option<usize>,
    page_size: Option<usize>,
    expands: Vec<ExpandBuilder>,
    include_count: bool,
}

impl<'a> QueryBuilder<'a> {
    /// Creates a new query builder for the given entity.
    pub(crate) fn new(client: &'a DataverseClient, entity: Entity) -> Self {
        Self {
            client,
            entity,
            select: Vec::new(),
            filter: None,
            order_by: None,
            top: None,
            page_size: None,
            expands: Vec::new(),
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
    ///
    /// Accepts both [`Filter`] and [`ODataFilter`] (for negated filters).
    pub fn filter(mut self, filter: impl Into<ODataFilter>) -> Self {
        self.filter = Some(filter.into());
        self
    }

    /// Sets the ordering of results.
    pub fn order_by(mut self, order: OrderBy) -> Self {
        self.order_by = Some(order);
        self
    }

    /// Limits the total number of records returned.
    ///
    /// This is applied server-side and limits the total results across all pages.
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

    /// Expands a navigation property to include related records.
    ///
    /// # Example
    ///
    /// ```ignore
    /// client.query(Entity::logical("account"))
    ///     .select(&["name"])
    ///     .expand("primarycontactid", |e| {
    ///         e.select(&["fullname", "emailaddress1"])
    ///     })
    ///     .expand("contact_customer_accounts", |e| {
    ///         e.select(&["fullname"])
    ///          .filter(Filter::eq("statecode", 0))
    ///          .top(5)
    ///     });
    /// ```
    pub fn expand<F>(mut self, navigation_property: impl Into<String>, build: F) -> Self
    where
        F: FnOnce(ExpandBuilder) -> ExpandBuilder,
    {
        let expand = build(ExpandBuilder::new(navigation_property));
        self.expands.push(expand);
        self
    }

    /// Includes the total count of matching records in the response.
    ///
    /// When enabled, `Page::total_count()` will return the total number of
    /// records matching the query (not just the current page).
    pub fn include_count(mut self) -> Self {
        self.include_count = true;
        self
    }

    /// Builds the OData query URL.
    pub(crate) fn build_url(&self, entity_set_name: &str) -> String {
        let base_url = self.client.base_url().trim_end_matches('/');
        let api_version = self.client.api_version();

        let mut url = format!("{}/api/data/{}/{}", base_url, api_version, entity_set_name);

        let mut params = Vec::new();

        // $select
        if !self.select.is_empty() {
            params.push(format!("$select={}", self.select.join(",")));
        }

        // $filter
        if let Some(ref filter) = self.filter {
            params.push(format!("$filter={}", odata_filter_to_string(filter)));
        }

        // $orderby
        if let Some(ref order) = self.order_by {
            params.push(format!("$orderby={}", order_to_odata(order)));
        }

        // $top
        if let Some(top) = self.top {
            params.push(format!("$top={}", top));
        }

        // $expand
        if !self.expands.is_empty() {
            let expand_clauses: Vec<_> = self.expands.iter().map(|e| e.to_odata()).collect();
            params.push(format!("$expand={}", expand_clauses.join(",")));
        }

        // $count
        if self.include_count {
            params.push("$count=true".to_string());
        }

        // Prefer header handles page size, but we can also use $top for first page
        // Page size is handled via Prefer header in the request

        if !params.is_empty() {
            url.push('?');
            url.push_str(&params.join("&"));
        }

        url
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

    /// Executes a count query and returns the number of matching records.
    ///
    /// This uses the `$count` endpoint for efficiency.
    pub async fn count(self) -> Result<usize, Error> {
        let entity_set_name = self.resolve_entity_set().await?;
        let base_url = self.client.base_url().trim_end_matches('/');
        let api_version = self.client.api_version();

        let mut url = format!(
            "{}/api/data/{}/{}/$count",
            base_url, api_version, entity_set_name
        );

        // Only $filter is supported with $count
        if let Some(ref filter) = self.filter {
            url.push_str(&format!("?$filter={}", odata_filter_to_string(filter)));
        }

        let response: reqwest::Response = self
            .client
            .request(reqwest::Method::GET, &url, None, None)
            .await?;

        let count_text = response.text().await.map_err(crate::error::ApiError::from)?;
        let count: usize = count_text.trim().parse().map_err(|_| {
            Error::Api(crate::error::ApiError::Parse {
                message: format!("Invalid count response: {}", count_text),
                body: Some(count_text),
            })
        })?;

        Ok(count)
    }

    /// Converts this query builder into an async iterator over pages.
    pub fn into_async_iter(self) -> ODataPages<'a> {
        ODataPages::new(self)
    }
}
