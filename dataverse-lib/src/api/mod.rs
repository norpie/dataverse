//! Web API operations

mod aggregate;
mod association;
mod async_operation;
mod batch;
mod crud;
mod execute;
mod forms;
mod metadata;
mod options;
pub mod query;
pub mod schema;
mod views;

pub use aggregate::*;
pub use association::*;
pub use async_operation::*;
pub use batch::*;
pub use crud::*;
pub use execute::*;
pub use forms::*;
pub use metadata::*;
pub use options::*;
pub use views::*;
