//! UI components with self-managed state.
//!
//! Each component lives in its own module with:
//! - `state.rs` - the component state type
//! - `render.rs` - rendering logic
//! - `mod.rs` - public exports

pub mod input;

pub use input::{Input, InputId};
