//! OData query builder.

use std::collections::HashMap;
use std::collections::HashSet;

use crate::DataverseClient;
use crate::api::query::Filter;
use crate::api::query::ODataFilter;
use crate::api::query::OrderBy;
use crate::error::Error;
use crate::model::Entity;
use crate::model::Record;

use super::expand::ExpandBuilder;
use super::pages::ODataPages;
use super::url::build_select_expand_params;
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
#[derive(Clone)]
pub struct QueryBuilder {
    entity: Entity,
    select: Vec<String>,
    filter: Option<ODataFilter>,
    order_by: Option<OrderBy>,
    top: Option<usize>,
    page_size: Option<usize>,
    expands: Vec<ExpandBuilder>,
    include_count: bool,
    bypass_cache: bool,
}

impl QueryBuilder {
    /// Creates a new query builder for the given entity.
    pub fn new(entity: Entity) -> Self {
        Self {
            entity,
            select: Vec::new(),
            filter: None,
            order_by: None,
            top: None,
            page_size: None,
            expands: Vec::new(),
            include_count: false,
            bypass_cache: false,
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

    /// Bypasses the query cache for this query.
    ///
    /// By default, OData query results are cached using the client's cache
    /// provider with the configured `query_ttl`. Call this to force a fresh
    /// fetch from the server.
    pub fn bypass_cache(mut self) -> Self {
        self.bypass_cache = true;
        self
    }

    /// Transforms lookup field names to OData format (`_fieldname_value`).
    ///
    /// This fetches entity metadata to identify lookup fields and transforms
    /// field names in select, filter, and order_by clauses.
    pub(crate) async fn transform_lookup_fields(
        &mut self,
        client: &DataverseClient,
        entity_logical_name: &str,
    ) -> Result<(), Error> {
        // Fetch attribute metadata for the entity
        let attributes = client
            .metadata()
            .attributes(Entity::logical(entity_logical_name))
            .await?;

        // Build a set of lookup field names for quick lookup
        let lookup_fields: HashSet<String> = attributes
            .iter()
            .filter(|attr| attr.is_lookup())
            .map(|attr| attr.logical_name.clone())
            .collect();

        // Build a mapping from lookup logical name → schema name for nav properties.
        // In Dataverse OData, navigation property names use the SchemaName (PascalCase),
        // not the LogicalName (lowercase). e.g., "nrq_projectid" → "nrq_ProjectId".
        let lookup_nav_names: HashMap<String, String> = attributes
            .iter()
            .filter(|attr| attr.is_lookup())
            .map(|attr| (attr.logical_name.clone(), attr.schema_name.clone()))
            .collect();

        // Transform select fields
        self.select = self
            .select
            .iter()
            .map(|field| transform_field_name(field, &lookup_fields))
            .collect();

        // Transform filter fields
        if let Some(ref filter) = self.filter {
            self.filter = Some(transform_odata_filter(filter, &lookup_fields));
        }

        // Transform order_by fields
        if let Some(ref order) = self.order_by {
            self.order_by = Some(transform_order_by(order, &lookup_fields));
        }

        // Transform expand navigation property names from logical to schema names
        transform_expand_nav_properties(&mut self.expands, &lookup_nav_names);

        Ok(())
    }

    /// Builds the OData query URL.
    pub(crate) fn build_url(&self, client: &DataverseClient, entity_set_name: &str) -> String {
        let base_url = client.base_url().trim_end_matches('/');
        let api_version = client.api_version();

        let mut url = format!("{}/api/data/{}/{}", base_url, api_version, entity_set_name);

        let mut params = Vec::new();

        // Sort select fields for deterministic URL generation (important for caching)
        let mut sorted_select = self.select.clone();
        sorted_select.sort();

        // $select and $expand (using shared helper)
        let select_expand = build_select_expand_params(&sorted_select, &self.expands);
        if !select_expand.is_empty() {
            params.push(select_expand);
        }

        // $filter
        if let Some(ref filter) = self.filter {
            let filter_str = odata_filter_to_string(filter);
            params.push(format!("$filter={}", urlencoding::encode(&filter_str)));
        }

        // $orderby
        if let Some(ref order) = self.order_by {
            let orderby_str = order_to_odata(order);
            params.push(format!("$orderby={}", urlencoding::encode(&orderby_str)));
        }

        // $top
        if let Some(top) = self.top {
            params.push(format!("$top={}", top));
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

    /// Returns whether caching is bypassed for this query.
    pub(crate) fn bypass_cache_value(&self) -> bool {
        self.bypass_cache
    }

    /// Returns a reference to the entity.
    pub fn entity(&self) -> &Entity {
        &self.entity
    }

    /// Returns the selected fields.
    ///
    /// Empty if no `.select()` was called (meaning all fields).
    pub fn selected_fields(&self) -> &[String] {
        &self.select
    }

    /// Resolves the entity to its entity set name.
    async fn resolve_entity_set(&self, client: &DataverseClient) -> Result<String, Error> {
        match &self.entity {
            Entity::Set(name) => Ok(name.clone()),
            Entity::Logical(logical_name) => client.resolve_entity_set_name(logical_name).await,
        }
    }

    /// Executes the query and returns the first page of results.
    ///
    /// Use `into_async_iter()` to iterate over all pages.
    pub async fn execute(self, client: &DataverseClient) -> Result<Vec<Record>, Error> {
        let mut pages = self.into_async_iter(client);
        match pages.next(client).await {
            Some(Ok(page)) => Ok(page.into_records()),
            Some(Err(e)) => Err(e),
            None => Ok(Vec::new()),
        }
    }

    /// Executes the query and returns the first matching record.
    pub async fn first(self, client: &DataverseClient) -> Result<Option<Record>, Error> {
        // Optimize by limiting to 1 record
        let builder = Self {
            top: Some(1),
            ..self
        };
        let records = builder.execute(client).await?;
        Ok(records.into_iter().next())
    }

    /// Executes a count query and returns the number of matching records.
    ///
    /// Uses `$apply=aggregate($count as count)` which returns accurate counts
    /// (the `/$count` endpoint is limited to 5000 on some Dataverse instances).
    pub async fn count(mut self, client: &DataverseClient) -> Result<usize, Error> {
        let entity_set_name = self.resolve_entity_set(client).await?;

        // Resolve logical name for lookup field transformation
        let entity_logical_name = match &self.entity {
            Entity::Logical(name) => name.clone(),
            Entity::Set(name) => {
                client
                    .resolve_entity_logical_name(&Entity::Set(name.clone()))
                    .await?
            }
        };

        // Transform lookup field names in filter before building URL
        self.transform_lookup_fields(client, &entity_logical_name)
            .await?;

        let base_url = client.base_url().trim_end_matches('/');
        let api_version = client.api_version();

        let mut url = format!(
            "{}/api/data/{}/{}?$apply=aggregate($count as count)",
            base_url, api_version, entity_set_name
        );

        // Add filter if present
        if let Some(ref filter) = self.filter {
            let filter_str = odata_filter_to_string(filter);
            url.push_str(&format!("&$filter={}", urlencoding::encode(&filter_str)));
        }

        log::debug!("[QueryBuilder] Count URL: {}", url);

        // Build OData headers
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "OData-MaxVersion",
            reqwest::header::HeaderValue::from_static("4.0"),
        );
        headers.insert(
            "OData-Version",
            reqwest::header::HeaderValue::from_static("4.0"),
        );
        headers.insert(
            "Accept",
            reqwest::header::HeaderValue::from_static("application/json"),
        );

        let response: reqwest::Response = client
            .request(reqwest::Method::GET, &url, Some(headers), None)
            .await?;

        // Parse JSON response: {"value": [{"count": 12345}]}
        let json: serde_json::Value = response
            .json()
            .await
            .map_err(crate::error::ApiError::from)?;

        let count = json
            .get("value")
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .and_then(|obj| obj.get("count"))
            .and_then(|c| c.as_u64())
            .ok_or_else(|| {
                Error::Api(crate::error::ApiError::Parse {
                    message: "Invalid count response format".to_string(),
                    body: Some(json.to_string()),
                })
            })?;

        Ok(count as usize)
    }

    /// Converts this query builder into an async iterator over pages.
    pub fn into_async_iter(self, client: &DataverseClient) -> ODataPages {
        ODataPages::new(self, client)
    }
}

/// Transforms a field name to OData lookup format if it's a lookup field.
fn transform_field_name(field: &str, lookup_fields: &HashSet<String>) -> String {
    if lookup_fields.contains(field) {
        format!("_{}_value", field)
    } else {
        field.to_string()
    }
}

/// Transforms field names in an ODataFilter.
fn transform_odata_filter(filter: &ODataFilter, lookup_fields: &HashSet<String>) -> ODataFilter {
    match filter {
        ODataFilter::Base(f) => ODataFilter::Base(transform_filter(f, lookup_fields)),
        ODataFilter::Not(inner) => {
            ODataFilter::Not(Box::new(transform_odata_filter(inner, lookup_fields)))
        }
    }
}

/// Transforms field names in a Filter.
fn transform_filter(filter: &Filter, lookup_fields: &HashSet<String>) -> Filter {
    match filter {
        Filter::Eq(field, value) => {
            Filter::Eq(transform_field_name(field, lookup_fields), value.clone())
        }
        Filter::Ne(field, value) => {
            Filter::Ne(transform_field_name(field, lookup_fields), value.clone())
        }
        Filter::Gt(field, value) => {
            Filter::Gt(transform_field_name(field, lookup_fields), value.clone())
        }
        Filter::Ge(field, value) => {
            Filter::Ge(transform_field_name(field, lookup_fields), value.clone())
        }
        Filter::Lt(field, value) => {
            Filter::Lt(transform_field_name(field, lookup_fields), value.clone())
        }
        Filter::Le(field, value) => {
            Filter::Le(transform_field_name(field, lookup_fields), value.clone())
        }
        Filter::Contains(field, value) => {
            Filter::Contains(transform_field_name(field, lookup_fields), value.clone())
        }
        Filter::StartsWith(field, value) => {
            Filter::StartsWith(transform_field_name(field, lookup_fields), value.clone())
        }
        Filter::EndsWith(field, value) => {
            Filter::EndsWith(transform_field_name(field, lookup_fields), value.clone())
        }
        Filter::IsNull(field) => Filter::IsNull(transform_field_name(field, lookup_fields)),
        Filter::IsNotNull(field) => Filter::IsNotNull(transform_field_name(field, lookup_fields)),
        Filter::And(filters) => Filter::And(
            filters
                .iter()
                .map(|f| transform_filter(f, lookup_fields))
                .collect(),
        ),
        Filter::Or(filters) => Filter::Or(
            filters
                .iter()
                .map(|f| transform_filter(f, lookup_fields))
                .collect(),
        ),
        Filter::Not(inner) => Filter::Not(Box::new(transform_filter(inner, lookup_fields))),
        Filter::Raw(s) => Filter::Raw(s.clone()),
    }
}

/// Transforms expand navigation property names from logical to schema names.
///
/// In Dataverse OData, navigation property names use the SchemaName (PascalCase),
/// not the LogicalName (lowercase). For example, a lookup attribute with logical
/// name `nrq_projectid` has navigation property name `nrq_ProjectId`.
///
/// This only transforms the top-level navigation property names using the provided
/// entity's lookup metadata. Nested expand contents (select/filter) would need
/// metadata from the expanded entity and are not transformed here.
fn transform_expand_nav_properties(
    expands: &mut [ExpandBuilder],
    lookup_nav_names: &HashMap<String, String>,
) {
    for expand in expands.iter_mut() {
        let nav = expand.navigation_property();
        if let Some(schema_name) = lookup_nav_names.get(nav) {
            if nav != schema_name {
                log::debug!(
                    "[QueryBuilder] Transforming expand nav property: {} → {}",
                    nav,
                    schema_name
                );
                expand.set_navigation_property(schema_name.clone());
            }
        }
    }
}

/// Transforms field names in an OrderBy.
fn transform_order_by(order: &OrderBy, lookup_fields: &HashSet<String>) -> OrderBy {
    OrderBy {
        fields: order
            .fields
            .iter()
            .map(|(field, dir)| (transform_field_name(field, lookup_fields), *dir))
            .collect(),
    }
}
