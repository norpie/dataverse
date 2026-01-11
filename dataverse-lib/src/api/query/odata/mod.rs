//! OData query support.
//!
//! This module provides the OData query builder and URL generation for
//! querying Dataverse using OData syntax.
//!
//! # Example
//!
//! ```ignore
//! use dataverse_lib::api::query::odata::QueryBuilder;
//! use dataverse_lib::api::query::Filter;
//!
//! let mut pages = client.query(Entity::logical("account"))
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
mod expand;
mod pages;
pub(crate) mod url;

pub use builder::QueryBuilder;
pub use expand::ExpandBuilder;
pub use pages::ODataPages;
