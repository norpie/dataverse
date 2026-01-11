//! FetchXML query support.
//!
//! This module provides the FetchXML query builder and XML generation for
//! querying Dataverse using FetchXML syntax.
//!
//! FetchXML is a proprietary query language used by Microsoft Dataverse that
//! provides more advanced querying capabilities than OData, including:
//!
//! - Complex aggregations (group by, sum, count, avg, etc.)
//! - Link entities for joining related tables
//! - Paging with cookies for reliable pagination
//! - Distinct queries
//!
//! # Example
//!
//! ```ignore
//! use dataverse_lib::api::query::fetchxml::FetchBuilder;
//! use dataverse_lib::api::query::Filter;
//!
//! let mut pages = client.fetch(Entity::logical("account"))
//!     .select(&["name", "revenue"])
//!     .filter(Filter::gt("revenue", 1000000))
//!     .into_async_iter();
//!
//! while let Some(page) = pages.next().await {
//!     let page = page?;
//!     for record in page.records() {
//!         println!("{:?}", record);
//!     }
//! }
//! ```

mod builder;
mod link;
mod pages;
pub(crate) mod xml;

pub use builder::FetchBuilder;
pub use link::LinkEntityBuilder;
pub use link::LinkType;
pub use pages::FetchXmlPages;
