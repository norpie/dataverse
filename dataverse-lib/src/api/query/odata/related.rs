//! Related records query builder and pagination.

use reqwest::header::HeaderMap;
use reqwest::header::HeaderValue;
use reqwest::Method;
use serde::Deserialize;
use uuid::Uuid;

use super::expand::ExpandBuilder;
use super::url::build_select_expand_params;
use super::url::odata_filter_to_string;
use super::url::order_to_odata;
use crate::api::query::ODataFilter;
use crate::api::query::OrderBy;
use crate::api::query::Page;
use crate::error::ApiError;
use crate::error::Error;
use crate::model::Entity;
use crate::model::Record;
use crate::DataverseClient;

/// Builder for querying records related through a navigation property.
///
/// Use [`DataverseClient::related`] to create a related query builder.
///
/// # Example
///
/// ```ignore
/// // Query contacts related to an account
/// let mut pages = client.related(
///     Entity::set("accounts"), account_id,
///     "contact_customer_accounts",
/// )
/// .select(&["fullname", "emailaddress1"])
/// .filter(Filter::eq("statecode", 0))
/// .order_by(OrderBy::asc("fullname"))
/// .into_async_iter();
///
/// while let Some(page) = pages.next().await {
///     let page = page?;
///     for record in page.records() {
///         println!("{:?}", record);
///     }
/// }
/// ```
pub struct RelatedQueryBuilder<'a> {
    client: &'a DataverseClient,
    entity: Entity,
    id: Uuid,
    nav_property: String,
    select: Vec<String>,
    filter: Option<ODataFilter>,
    order_by: Option<OrderBy>,
    top: Option<usize>,
    page_size: Option<usize>,
    expands: Vec<ExpandBuilder>,
    include_count: bool,
}

impl<'a> RelatedQueryBuilder<'a> {
    /// Creates a new related query builder.
    pub(crate) fn new(
        client: &'a DataverseClient,
        entity: Entity,
        id: Uuid,
        nav_property: impl Into<String>,
    ) -> Self {
        Self {
            client,
            entity,
            id,
            nav_property: nav_property.into(),
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

    /// Expands a navigation property on the related records.
    pub fn expand(
        mut self,
        nav_property: impl Into<String>,
        configure: impl FnOnce(ExpandBuilder) -> ExpandBuilder,
    ) -> Self {
        let expand = configure(ExpandBuilder::new(nav_property));
        self.expands.push(expand);
        self
    }

    /// Includes the total count of matching records in the response.
    pub fn count(mut self) -> Self {
        self.include_count = true;
        self
    }

    /// Converts this builder into an async page iterator.
    pub fn into_async_iter(self) -> RelatedPages<'a> {
        RelatedPages::new(self)
    }

    /// Executes the query and returns the first page of results.
    pub async fn execute(self) -> Result<Vec<Record>, Error> {
        let mut pages = self.into_async_iter();
        match pages.next().await {
            Some(Ok(page)) => Ok(page.into_records()),
            Some(Err(e)) => Err(e),
            None => Ok(Vec::new()),
        }
    }

    /// Executes the query and returns the first matching record.
    pub async fn first(mut self) -> Result<Option<Record>, Error> {
        self.top = Some(1);
        let records = self.execute().await?;
        Ok(records.into_iter().next())
    }

    /// Executes a count-only query.
    ///
    /// Returns the total number of records matching the filter.
    pub async fn count_only(mut self) -> Result<usize, Error> {
        self.include_count = true;
        self.top = Some(0);
        let mut pages = self.into_async_iter();
        match pages.next().await {
            Some(Ok(page)) => Ok(page.total_count().unwrap_or(0)),
            Some(Err(e)) => Err(e),
            None => Ok(0),
        }
    }

    /// Returns the client reference.
    pub(crate) fn client(&self) -> &'a DataverseClient {
        self.client
    }

    /// Returns the entity.
    pub(crate) fn entity(&self) -> &Entity {
        &self.entity
    }

    /// Returns the page size value.
    pub(crate) fn page_size_value(&self) -> Option<usize> {
        self.page_size
    }

    /// Builds the query URL (without base URL).
    ///
    /// The entity_set_name is the resolved entity set name for the source entity.
    pub(crate) fn build_url(&self, entity_set_name: &str) -> String {
        // Base path: /{entity_set}({id})/{nav_property}
        let mut url = format!(
            "/{}({})/{}",
            entity_set_name, self.id, self.nav_property
        );

        // Build query parameters
        let mut params = Vec::new();

        // $select and $expand
        let select_expand = build_select_expand_params(&self.select, &self.expands);
        if !select_expand.is_empty() {
            params.push(select_expand);
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

        // $count
        if self.include_count {
            params.push("$count=true".to_string());
        }

        // Append query string
        if !params.is_empty() {
            url.push('?');
            url.push_str(&params.join("&"));
        }

        url
    }
}

// =============================================================================
// RelatedPages - Async iterator for related record pagination
// =============================================================================

/// Async iterator that yields pages of related record query results.
///
/// Automatically follows `@odata.nextLink` for pagination.
pub struct RelatedPages<'a> {
    client: &'a DataverseClient,
    /// The initial URL (built from query builder).
    initial_url: Option<String>,
    /// Page size preference.
    page_size: Option<usize>,
    /// The next URL to fetch (from @odata.nextLink).
    next_url: Option<String>,
    /// Whether we've exhausted all pages.
    done: bool,
    /// Builder for first call (needs entity resolution).
    needs_resolution: Option<RelatedQueryBuilder<'a>>,
}

impl<'a> RelatedPages<'a> {
    /// Creates a new async iterator from a related query builder.
    pub(crate) fn new(builder: RelatedQueryBuilder<'a>) -> Self {
        let client = builder.client();
        let page_size = builder.page_size_value();

        Self {
            client,
            initial_url: None,
            page_size,
            next_url: None,
            done: false,
            needs_resolution: Some(builder),
        }
    }

    /// Fetches the next page of results.
    ///
    /// Returns `None` when all pages have been consumed.
    pub async fn next(&mut self) -> Option<Result<Page, Error>> {
        if self.done {
            return None;
        }

        // Determine which URL to fetch
        let url = if let Some(builder) = self.needs_resolution.take() {
            // First call: resolve entity and build URL
            let entity_set_name = match builder.entity() {
                Entity::Set(name) => name.clone(),
                Entity::Logical(logical_name) => {
                    match self.client.resolve_entity_set_name(logical_name).await {
                        Ok(name) => name,
                        Err(e) => {
                            self.done = true;
                            return Some(Err(e));
                        }
                    }
                }
            };
            let relative_url = builder.build_url(&entity_set_name);
            let full_url = self.client.build_url(&relative_url);
            self.initial_url = Some(full_url.clone());
            full_url
        } else if let Some(url) = self.next_url.take() {
            // Subsequent pages: use nextLink (already a full URL)
            url
        } else if let Some(url) = self.initial_url.take() {
            // First page with pre-resolved URL
            url
        } else {
            // No more pages
            self.done = true;
            return None;
        };

        // Build headers
        let mut headers = HeaderMap::new();
        headers.insert("OData-MaxVersion", HeaderValue::from_static("4.0"));
        headers.insert("OData-Version", HeaderValue::from_static("4.0"));
        headers.insert("Accept", HeaderValue::from_static("application/json"));
        headers.insert(
            "Prefer",
            HeaderValue::from_static("odata.include-annotations=\"*\""),
        );

        // Add page size preference if specified
        if let Some(size) = self.page_size {
            headers.insert(
                "Prefer",
                HeaderValue::from_str(&format!(
                    "odata.include-annotations=\"*\",odata.maxpagesize={}",
                    size
                ))
                .unwrap_or_else(|_| HeaderValue::from_static("odata.include-annotations=\"*\"")),
            );
        }

        // Make request
        let response = match self.client.request(Method::GET, &url, headers, None).await {
            Ok(resp) => resp,
            Err(e) => {
                self.done = true;
                return Some(Err(e));
            }
        };

        // Parse response
        let odata_response: ODataResponse = match response.json().await {
            Ok(resp) => resp,
            Err(e) => {
                self.done = true;
                return Some(Err(Error::Api(ApiError::from(e))));
            }
        };

        // Build page
        let mut page = Page::new(odata_response.value);

        if let Some(count) = odata_response.count {
            page = page.with_total_count(count);
        }

        if let Some(next_link) = odata_response.next_link {
            page = page.with_next_link(next_link.clone());
            self.next_url = Some(next_link);
        } else {
            self.done = true;
        }

        Some(Ok(page))
    }
}

/// OData response structure for collection queries.
#[derive(Debug, Deserialize)]
struct ODataResponse {
    /// The records in this page.
    value: Vec<Record>,
    /// Total count (when $count=true).
    #[serde(rename = "@odata.count")]
    count: Option<usize>,
    /// Link to the next page.
    #[serde(rename = "@odata.nextLink")]
    next_link: Option<String>,
}
