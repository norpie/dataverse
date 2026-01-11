//! Async iterator for OData query pagination.

use reqwest::header::HeaderMap;
use reqwest::header::HeaderValue;
use reqwest::Method;
use serde::Deserialize;

use crate::api::query::Page;
use crate::error::ApiError;
use crate::error::Error;
use crate::model::Record;
use crate::DataverseClient;

use super::builder::QueryBuilder;

/// Async iterator that yields pages of OData query results.
///
/// Automatically follows `@odata.nextLink` for pagination.
///
/// # Example
///
/// ```ignore
/// let mut pages = client.query(Entity::logical("account"))
///     .select(&["name"])
///     .into_async_iter();
///
/// while let Some(page) = pages.next().await {
///     let page = page?;
///     for record in page.records() {
///         println!("{:?}", record);
///     }
/// }
/// ```
pub struct ODataPages<'a> {
    /// Reference to the client for making requests.
    client: &'a DataverseClient,
    /// The initial URL (built from query builder).
    initial_url: Option<String>,
    /// Page size preference.
    page_size: Option<usize>,
    /// The next URL to fetch (from @odata.nextLink).
    next_url: Option<String>,
    /// Whether we've exhausted all pages.
    done: bool,
    /// Whether we need to resolve the entity first.
    needs_resolution: Option<QueryBuilder<'a>>,
}

impl<'a> ODataPages<'a> {
    /// Creates a new async iterator from a query builder.
    pub(crate) fn new(builder: QueryBuilder<'a>) -> Self {
        // Extract values before moving builder
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
                crate::model::Entity::Set(name) => name.clone(),
                crate::model::Entity::Logical(logical_name) => {
                    match self.client.resolve_entity_set_name(logical_name).await {
                        Ok(name) => name,
                        Err(e) => {
                            self.done = true;
                            return Some(Err(e));
                        }
                    }
                }
            };
            let url = builder.build_url(&entity_set_name);
            self.initial_url = Some(url.clone());
            url
        } else if let Some(url) = self.next_url.take() {
            // Subsequent pages: use nextLink
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

        // Add page size preference if specified (only for first page)
        if let Some(size) = self.page_size {
            if let Ok(_value) = HeaderValue::from_str(&format!("odata.maxpagesize={}", size)) {
                // Note: This overwrites the previous Prefer header
                // In practice, we'd want to combine them, but for simplicity:
                headers.insert(
                    "Prefer",
                    HeaderValue::from_str(&format!(
                        "odata.include-annotations=\"*\",odata.maxpagesize={}",
                        size
                    ))
                    .unwrap_or_else(|_| HeaderValue::from_static("odata.include-annotations=\"*\"")),
                );
            }
        }

        // Make request
        let response: reqwest::Response = match self.client.request(Method::GET, &url, headers, None).await {
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
