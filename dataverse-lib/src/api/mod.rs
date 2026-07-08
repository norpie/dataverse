//! Web API operations

mod aggregate;
mod association;
mod async_operation;
mod audit;
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
pub use audit::*;
pub use batch::*;
pub use crud::*;
pub use execute::*;
pub use metadata::*;
