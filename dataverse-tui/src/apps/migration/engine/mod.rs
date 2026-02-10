//! Transform execution engine.
//!
//! This module contains the core execution logic for running transforms
//! on source records to produce target field values.

pub mod condition;
mod executor;
pub mod materializer;
mod paths;
mod transforms;
mod types;

pub use executor::execute_chain;
pub use executor::execute_scoped_chain;
pub use executor::BranchItem;
pub use executor::ChainChildren;
pub use executor::ChainItem;
pub use executor::FindConditionItem;
pub use paths::FieldPath;
pub use paths::Segment;
pub use types::FindError;
pub use types::StubTargetCache;
pub use types::SystemVars;
pub use types::TargetCache;
pub use types::TransformContext;
pub use types::TransformError;
pub use types::TransformResult;
