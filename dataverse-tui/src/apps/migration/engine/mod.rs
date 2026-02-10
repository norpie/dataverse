//! Transform execution engine.
//!
//! This module contains the core execution logic for running transforms
//! on source records to produce target field values.

pub mod condition;
mod executor;
pub mod materializer;
mod paths;
pub mod record;
mod transforms;
mod types;
pub mod variables;

pub use paths::FieldPath;
pub use types::StubTargetCache;
pub use types::SystemVars;
pub use types::TransformContext;
pub use types::TransformError;
pub use types::TransformResult;
