//! Collapsible widget - an expandable/collapsible container.
//!
//! The `Collapsible` widget provides a container that can expand and collapse
//! to show or hide its content. It displays a header with an indicator (▶/▼)
//! and title, and conditionally renders children based on its expanded state.
//!
//! # Example
//!
//! ```ignore
//! #[app]
//! struct MyApp {
//!     details: Collapsible,
//! }
//!
//! #[app_impl]
//! impl MyApp {
//!     fn page(&self) -> Node {
//!         page! {
//!             column {
//!                 collapsible(bind: self.details, title: "Details") {
//!                     text { "Hidden content here..." }
//!                 }
//!             }
//!         }
//!     }
//! }
//! ```

mod events;
mod state;

pub use state::{Collapsible, CollapsibleId};
