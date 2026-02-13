//! Transform execution engine.
//!
//! This module contains the core execution logic for running transforms
//! on source records to produce target field values.

pub mod condition;
mod executor;
pub mod materializer;
mod paths;
pub mod record;
pub(crate) mod transforms;
mod types;
pub mod util;
pub mod variables;

pub use executor::BranchItem;
pub use executor::ChainChildren;
pub use executor::ChainItem;
pub use executor::FindConditionItem;
pub use executor::execute_chain;
pub use paths::FieldPath;
pub use types::FindCache;
pub use types::FindError;
pub use types::PathCache;
pub use types::StubFindCache;
pub use types::SystemVars;
pub use types::TransformContext;
pub use types::TransformError;
pub use types::TransformResult;
