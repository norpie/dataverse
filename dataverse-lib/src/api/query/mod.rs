//! Query builders for OData and FetchXML.
//!
//! This module provides types and builders for constructing queries against
//! Dataverse. It supports both OData and FetchXML query syntaxes.
//!
//! # Shared Types
//!
//! - [`Filter`] - Filter conditions used by both OData and FetchXML
//! - [`OrderBy`] - Ordering specification for query results
//! - [`Page`] - A page of query results with pagination info
//!
//! # Query Builders
//!
//! - [`odata`] - OData query builder (uses `$filter`, `$select`, `$expand`, etc.)
//! - [`fetchxml`] - FetchXML query builder (uses XML-based query language)

mod filter;
pub mod fetchxml;
pub mod odata;
mod order;
mod page;

pub use filter::Filter;
pub use filter::ODataFilter;
pub use order::Direction;
pub use order::OrderBy;
pub use page::Page;
