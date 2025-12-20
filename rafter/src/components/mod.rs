//! UI components (widgets) that manage their own state.
//!
//! Components are self-contained reactive types that:
//! - Manage their own internal state
//! - Provide imperative methods for reading/writing
//! - Are recognized by `#[app]` and `#[modal]` macros (not wrapped in `State<T>`)
//! - Are bound to views using `bind:` syntax

mod input;

pub use input::{Input, InputId};
