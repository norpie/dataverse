//! Web API operations

mod aggregate;
mod association;
mod async_operation;
mod batch;
mod crud;
mod execute;
mod fetch;
mod forms;
mod metadata;
mod options;
mod query;
pub mod schema;
mod views;

pub use aggregate::*;
pub use association::*;
pub use async_operation::*;
pub use batch::*;
pub use crud::*;
pub use execute::*;
pub use fetch::*;
pub use forms::*;
pub use metadata::*;
pub use options::*;
pub use query::*;
pub use views::*;
