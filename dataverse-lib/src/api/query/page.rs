//! Page type for paginated query results.

use crate::model::Record;

/// A page of query results with pagination information.
///
/// When iterating over query results, each page contains a batch of records
/// along with metadata needed to fetch the next page.
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
///
///     for record in page.records() {
///         println!("{:?}", record.get_string("name"));
///     }
///
///     if let Some(link) = page.next_link() {
///         println!("More results available: {}", link);
///     }
/// }
/// ```
#[derive(Debug, Clone)]
pub struct Page {
    records: Vec<Record>,
    /// OData `@odata.nextLink` for fetching the next page.
    next_link: Option<String>,
    /// FetchXML paging cookie for fetching the next page.
    paging_cookie: Option<String>,
    /// Total record count (if requested with `$count=true`).
    total_count: Option<usize>,
}

impl Page {
    /// Creates a new page with records and optional pagination info.
    pub fn new(records: Vec<Record>) -> Self {
        Self {
            records,
            next_link: None,
            paging_cookie: None,
            total_count: None,
        }
    }

    /// Sets the OData next link for pagination.
    pub fn with_next_link(mut self, next_link: impl Into<String>) -> Self {
        self.next_link = Some(next_link.into());
        self
    }

    /// Sets the FetchXML paging cookie for pagination.
    pub fn with_paging_cookie(mut self, paging_cookie: impl Into<String>) -> Self {
        self.paging_cookie = Some(paging_cookie.into());
        self
    }

    /// Sets the total record count.
    pub fn with_total_count(mut self, count: usize) -> Self {
        self.total_count = Some(count);
        self
    }

    /// Returns a reference to the records in this page.
    pub fn records(&self) -> &[Record] {
        &self.records
    }

    /// Consumes the page and returns the records.
    pub fn into_records(self) -> Vec<Record> {
        self.records
    }

    /// Returns the OData next link for fetching the next page, if available.
    ///
    /// This is populated when using OData queries and more results are available.
    pub fn next_link(&self) -> Option<&str> {
        self.next_link.as_deref()
    }

    /// Returns the FetchXML paging cookie for fetching the next page, if available.
    ///
    /// This is populated when using FetchXML queries and more results are available.
    pub fn paging_cookie(&self) -> Option<&str> {
        self.paging_cookie.as_deref()
    }

    /// Returns the total record count, if it was requested.
    ///
    /// For OData queries, this requires `$count=true` in the request.
    pub fn total_count(&self) -> Option<usize> {
        self.total_count
    }

    /// Returns `true` if this page has no records.
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Returns the number of records in this page.
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Returns `true` if there are more pages available.
    ///
    /// This checks both OData next link and FetchXML paging cookie.
    pub fn has_more(&self) -> bool {
        self.next_link.is_some() || self.paging_cookie.is_some()
    }
}
