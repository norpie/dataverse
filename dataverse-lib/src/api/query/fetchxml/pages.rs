//! Async iterator for FetchXML query pagination.

use reqwest::header::HeaderMap;
use reqwest::header::HeaderValue;
use reqwest::Method;
use serde::Deserialize;
use url::form_urlencoded;

use crate::api::query::Page;
use crate::error::ApiError;
use crate::error::Error;
use crate::model::Record;
use crate::DataverseClient;

use super::builder::FetchBuilder;

/// Async iterator that yields pages of FetchXML query results.
///
/// Automatically handles paging cookies for pagination.
///
/// # Example
///
/// ```ignore
/// let mut pages = client.fetch(Entity::logical("account"))
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
pub struct FetchXmlPages<'a> {
    /// Reference to the client for making requests.
    client: &'a DataverseClient,
    /// The entity set name (resolved from entity).
    entity_set_name: Option<String>,
    /// The base FetchXML (without paging).
    base_fetchxml: Option<String>,
    /// Current page number.
    page_number: usize,
    /// Paging cookie from the last response.
    paging_cookie: Option<String>,
    /// Whether we've exhausted all pages.
    done: bool,
    /// Whether we need to resolve the entity first.
    needs_resolution: Option<FetchBuilder<'a>>,
}

impl<'a> FetchXmlPages<'a> {
    /// Creates a new async iterator from a fetch builder.
    pub(crate) fn new(builder: FetchBuilder<'a>) -> Self {
        // Extract values before moving builder
        let client = builder.client();

        Self {
            client,
            entity_set_name: None,
            base_fetchxml: None,
            page_number: 1,
            paging_cookie: None,
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

        // Resolve entity and build FetchXML on first call
        if let Some(builder) = self.needs_resolution.take() {
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
            self.entity_set_name = Some(entity_set_name);
            self.base_fetchxml = Some(builder.build_fetchxml());
        }

        let entity_set_name = self.entity_set_name.as_ref()?;
        let base_fetchxml = self.base_fetchxml.as_ref()?;

        // Build the FetchXML with paging info
        let fetchxml = self.build_paged_fetchxml(base_fetchxml);

        // Build URL
        let base_url = self.client.base_url().trim_end_matches('/');
        let api_version = self.client.api_version();
        let encoded_fetchxml: String = form_urlencoded::byte_serialize(fetchxml.as_bytes()).collect();
        let url = format!(
            "{}/api/data/{}/{}?fetchXml={}",
            base_url,
            api_version,
            entity_set_name,
            encoded_fetchxml
        );

        // Build headers
        let mut headers = HeaderMap::new();
        headers.insert("OData-MaxVersion", HeaderValue::from_static("4.0"));
        headers.insert("OData-Version", HeaderValue::from_static("4.0"));
        headers.insert("Accept", HeaderValue::from_static("application/json"));
        headers.insert(
            "Prefer",
            HeaderValue::from_static("odata.include-annotations=\"*\""),
        );

        // Make request
        let response: reqwest::Response =
            match self.client.request(Method::GET, &url, headers, None).await {
                Ok(resp) => resp,
                Err(e) => {
                    self.done = true;
                    return Some(Err(e));
                }
            };

        // Parse response
        let fetchxml_response: FetchXmlResponse = match response.json().await {
            Ok(resp) => resp,
            Err(e) => {
                self.done = true;
                return Some(Err(Error::Api(ApiError::from(e))));
            }
        };

        // Build page
        let mut page = Page::new(fetchxml_response.value);

        if let Some(cookie) = fetchxml_response.paging_cookie {
            page = page.with_paging_cookie(cookie.clone());
            self.paging_cookie = Some(cookie);
            self.page_number += 1;
        } else {
            // No more pages
            self.done = true;
        }

        if let Some(count) = fetchxml_response.total_record_count {
            page = page.with_total_count(count);
        }

        // Check if more records available
        if !fetchxml_response.more_records.unwrap_or(false) {
            self.done = true;
        }

        Some(Ok(page))
    }

    /// Builds the FetchXML with paging information.
    fn build_paged_fetchxml(&self, base: &str) -> String {
        if self.page_number == 1 && self.paging_cookie.is_none() {
            return base.to_string();
        }

        // Insert paging attributes into the fetch element
        let page_attr = format!(r#" page="{}""#, self.page_number);
        let cookie_attr = self
            .paging_cookie
            .as_ref()
            .map(|c| format!(r#" paging-cookie="{}""#, xml_escape_attr(c)))
            .unwrap_or_default();

        // Find the end of the <fetch ...> opening tag and insert paging attributes
        if let Some(pos) = base.find('>') {
            let (before, after) = base.split_at(pos);
            format!("{}{}{}{}", before, page_attr, cookie_attr, after)
        } else {
            base.to_string()
        }
    }
}

/// Escapes a string for use in XML attribute values (for paging cookie).
fn xml_escape_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// FetchXML response structure.
#[derive(Debug, Deserialize)]
struct FetchXmlResponse {
    /// The records in this page.
    value: Vec<Record>,
    /// Paging cookie for next page.
    #[serde(rename = "@Microsoft.Dynamics.CRM.fetchxmlpagingcookie")]
    paging_cookie: Option<String>,
    /// Total record count (when returntotalrecordcount="true").
    #[serde(rename = "@Microsoft.Dynamics.CRM.totalrecordcount")]
    total_record_count: Option<usize>,
    /// Whether more records are available.
    #[serde(rename = "@Microsoft.Dynamics.CRM.morerecords")]
    more_records: Option<bool>,
}
