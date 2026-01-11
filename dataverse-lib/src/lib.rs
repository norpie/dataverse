//! Dataverse API client library
//!
//! A Rust async client library for Microsoft Dynamics 365 Dataverse Web API (v9.2).

pub mod api;
pub mod auth;
pub mod cache;
pub mod error;
pub mod index;
pub mod model;
pub mod rate_limit;
pub mod response;
pub mod stream;

mod client;

pub use client::*;
pub use response::CacheStatus;
pub use response::Response;
