//! Async iterator for OData query pagination.

use reqwest::Method;
use reqwest::header::HeaderMap;
use reqwest::header::HeaderValue;
use serde::Deserialize;

use uuid::Uuid;

use crate::DataverseClient;
use crate::api::query::Page;
use crate::error::ApiError;
use crate::error::Error;
use crate::model::Record;
use crate::model::Value;

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
#[derive(Clone)]
pub struct ODataPages {
    /// The initial URL (built from query builder).
    initial_url: Option<String>,
    /// Page size preference.
    page_size: Option<usize>,
    /// The next URL to fetch (from @odata.nextLink).
    next_url: Option<String>,
    /// Whether we've exhausted all pages.
    done: bool,
    /// Whether we need to resolve the entity first.
    needs_resolution: Option<QueryBuilder>,
    /// Primary ID attribute name, resolved on first call.
    primary_id_attribute: Option<String>,
}

impl ODataPages {
    /// Creates a new async iterator from a query builder.
    pub(crate) fn new(builder: QueryBuilder, _client: &DataverseClient) -> Self {
        let page_size = builder.page_size_value();

        Self {
            initial_url: None,
            page_size,
            next_url: None,
            done: false,
            needs_resolution: Some(builder),
            primary_id_attribute: None,
        }
    }

    /// Fetches the next page of results.
    ///
    /// Returns `None` when all pages have been consumed.
    pub async fn next(&mut self, client: &DataverseClient) -> Option<Result<Page, Error>> {
        if self.done {
            return None;
        }

        // Determine which URL to fetch
        let url = if let Some(mut builder) = self.needs_resolution.take() {
            // First call: resolve entity and build URL
            let (entity_set_name, entity_logical_name) = match builder.entity().clone() {
                crate::model::Entity::Set(name) => {
                    // For entity set names, we need to resolve to logical name for metadata
                    match client
                        .resolve_entity_logical_name(&crate::model::Entity::Set(name.clone()))
                        .await
                    {
                        Ok(logical_name) => {
                            // Also resolve primary key from logical name
                            if let Ok((_, primary_id)) =
                                client.resolve_entity_core(&logical_name).await
                            {
                                self.primary_id_attribute = Some(primary_id);
                            }
                            (name, logical_name)
                        }
                        Err(e) => {
                            self.done = true;
                            return Some(Err(e));
                        }
                    }
                }
                crate::model::Entity::Logical(logical_name) => {
                    match client.resolve_entity_core(&logical_name).await {
                        Ok((set_name, primary_id)) => {
                            self.primary_id_attribute = Some(primary_id);
                            (set_name, logical_name)
                        }
                        Err(e) => {
                            self.done = true;
                            return Some(Err(e));
                        }
                    }
                }
            };

            // Transform lookup field names to OData format
            if let Err(e) = builder
                .transform_lookup_fields(client, &entity_logical_name)
                .await
            {
                self.done = true;
                return Some(Err(e));
            }

            let url = builder.build_url(client, &entity_set_name);
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
        if let Some(size) = self.page_size
            && let Ok(_value) = HeaderValue::from_str(&format!("odata.maxpagesize={}", size))
        {
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

        // Make request
        let response: reqwest::Response =
            match client.request(Method::GET, &url, headers, None).await {
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

        // Populate record IDs from the primary key field
        let mut records = odata_response.value;
        if let Some(ref pk_field) = self.primary_id_attribute {
            for record in &mut records {
                if record.id().is_none() {
                    if let Some(id) = record.get(pk_field).and_then(|v| match v {
                        Value::Guid(id) => Some(*id),
                        Value::String(s) => Uuid::parse_str(s).ok(),
                        _ => None,
                    }) {
                        record.set_id(id);
                    }
                }
            }
        }

        // Build page
        let mut page = Page::new(records);

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
